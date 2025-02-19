use std::{
  collections::{BTreeMap, BTreeSet, btree_map::Entry},
  hash::Hash,
  ops::{Add, AddAssign},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::log::{debug, error, info};

use crate::{
  Ballot, BallotChoice, Builder, Candidate, DuplicateCandidateMode, ElectionResult, ElectionStats,
  EliminationAlgorithm, EliminationStats, MaxSkippedRank, OverVoteRule, RoundStats, TieBreakMode, VoteRules,
  VotingErrors, VotingResult, Wrapper, archive::FetchTransactionResult, vote::BlockStatus,
};

// **** Private structures ****

type RoundId = u32;

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Ord, PartialOrd, Serialize, Deserialize)]
struct CandidateId(u32);

// A position in a ballot may not be filled with a candidate name, and this may
// still be acceptable. It simply means that this ballot will not be account for
// this turn.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Ord, PartialOrd, Serialize, Deserialize)]
enum Choice {
  BlankOrUndervote,
  Overvote,
  Undeclared,
  Filled(CandidateId),
}

// Invariant: there is at least one CandidateId in all the choices.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
struct RankedVoteCandidates {
  first_valid: CandidateId,
  rest: Vec<Choice>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct RankedVote {
  pub account: String,
  pub hash: String,
  pub memo: String,
  pub height: i64,
  pub status: BlockStatus,
  pub timestamp: i64,
  pub nonce: i64,
  pub proposals: Vec<String>,
}

impl RankedVote {
  pub fn new(
    account: impl Into<String>,
    hash: impl Into<String>,
    memo: impl Into<String>,
    height: i64,
    status: BlockStatus,
    timestamp: i64,
    nonce: i64,
  ) -> Self {
    Self {
      account: account.into(),
      hash: hash.into(),
      memo: memo.into(),
      height,
      status,
      timestamp,
      nonce,
      proposals: vec![],
    }
  }

  pub fn update_memo(&mut self, memo: impl Into<String>) {
    let memo = memo.into();
    self.memo = memo;
  }

  pub fn update_status(&mut self, status: BlockStatus) {
    self.status = status;
  }

  pub fn is_newer_than(&self, other: &RankedVote) -> bool {
    self.height > other.height || (self.height == other.height && self.nonce > other.nonce)
  }

  pub(crate) fn decode_memo(&self) -> Result<String> {
    let decoded =
      bs58::decode(&self.memo).into_vec().with_context(|| format!("failed to decode memo {} - bs58", &self.memo))?;

    let value = &decoded[3 .. decoded[2] as usize + 3];

    let result =
      String::from_utf8(value.to_vec()).with_context(|| format!("failed to decode memo {} - from_utf8", &self.memo))?;
    Ok(result)
  }

  pub fn parse_decoded_ranked_votes_memo(&mut self, key: &str) -> Option<(String, Vec<String>)> {
    if let Ok(decoded) = self.decode_memo() {
      let decoded = decoded.to_lowercase();

      // Split the decoded memo into parts by whitespace
      let mut parts = decoded.split_whitespace();

      // Check if the first part is the "MEF" prefix
      if let Some(prefix) = parts.next() {
        if prefix == "mef" {
          // Extract the round_id
          if let Some(round_id) = parts.next() {
            if round_id == key {
              // Collect remaining parts as proposal IDs
              let proposal_ids: Vec<String> = parts.map(|id| id.to_string()).collect();
              tracing::info!("decoded memo: {}", decoded);
              tracing::info!("proposals: {:?}", proposal_ids);
              return Some((round_id.to_string(), proposal_ids));
            }
          }
        }
      }
    }
    None
  }
}

impl RankedVoteCandidates {
  /// Removes all the eliminated candidates from the list of choices.
  /// Takes into account the policy for duplicated candidates. If the head
  /// candidates appears multiple time under the exhaust policy, this ballot
  /// will be exhausted.
  fn filtered_candidate(
    &self,
    still_valid: &BTreeSet<CandidateId>,
    duplicate_policy: DuplicateCandidateMode,
    overvote: OverVoteRule,
    skipped_ranks: MaxSkippedRank,
  ) -> Option<RankedVoteCandidates> {
    // If the top candidate did not get eliminated, keep the current ranked choice.
    if still_valid.contains(&self.first_valid) {
      return Some(self.clone());
    }

    // Run the choice pruning procedure.
    // Add again the first choice since it may have an impact on the elimination
    // rules.
    let mut all_choices = vec![Choice::Filled(self.first_valid)];
    all_choices.extend(self.rest.clone());

    if let Some((first_valid, rest)) =
      advance_voting(&all_choices, still_valid, duplicate_policy, overvote, skipped_ranks)
    {
      Some(RankedVoteCandidates { first_valid, rest })
    } else {
      None
    }
  }
}

impl From<FetchTransactionResult> for RankedVote {
  fn from(res: FetchTransactionResult) -> Self {
    RankedVote::new(res.account, res.hash, res.memo, res.height, res.status, res.timestamp, res.nonce)
  }
}
impl Wrapper<Vec<RankedVote>> {
  pub fn process_ranked_vote(self, id: usize, tip: i64) -> Wrapper<BTreeMap<String, RankedVote>> {
    let mut map = BTreeMap::new();
    let id_str = id.to_string();

    for mut vote in self.0 {
      // Use the updated `match_decoded_ranked_vote_memo` function
      if let Some((_round_id, proposal_ids)) = vote.parse_decoded_ranked_votes_memo(&id_str) {
        // Update the memo with proposal IDs
        vote.update_memo(format!("Votes: {:?}", proposal_ids));
        vote.proposals = proposal_ids;
        // Update vote status if conditions are met
        if tip - vote.height >= 10 {
          vote.update_status(BlockStatus::Canonical);
        }

        // Insert into the map based on account
        match map.entry(vote.account.clone()) {
          Entry::Vacant(e) => {
            e.insert(vote);
          }
          Entry::Occupied(mut e) => {
            let current_vote = e.get_mut();
            if !vote.is_newer_than(current_vote) {
              // Avoid updating the vote if it is newer
              *current_vote = vote;
            }
          }
        }
      }
    }

    Wrapper(map)
  }
}
#[derive(Eq, PartialEq, Debug, Clone, Copy, PartialOrd, Ord, Hash)]
struct VoteCount(u64);

impl VoteCount {
  const EMPTY: VoteCount = VoteCount(0);
}

impl std::iter::Sum for VoteCount {
  fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
    VoteCount(iter.map(|vc| vc.0).sum())
  }
}

impl AddAssign for VoteCount {
  fn add_assign(&mut self, rhs: VoteCount) {
    self.0 += rhs.0;
  }
}

impl Add for VoteCount {
  type Output = VoteCount;
  fn add(self: VoteCount, rhs: VoteCount) -> VoteCount {
    VoteCount(self.0 + rhs.0)
  }
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct VoteInternal {
  candidates: RankedVoteCandidates,
  count: VoteCount,
}

#[derive(Eq, PartialEq, Debug, Clone)]
enum RoundCandidateStatusInternal {
  StillRunning,
  Elected,
  /// if eliminated, the transfers of the votes to each candidate
  /// the last element is the number of exhausted votes
  Eliminated(Vec<(CandidateId, VoteCount)>, VoteCount),
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct InternalRoundStatistics {
  candidate_stats: Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)>,
  uwi_elimination_stats: Option<(Vec<(CandidateId, VoteCount)>, VoteCount)>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct RoundResult {
  votes: Vec<VoteInternal>,
  stats: InternalRoundStatistics,
  // Winning vote threshold
  vote_threshold: VoteCount,
}

/// Multi-winner proportional election using the instant-runoff voting
/// algorithm. Runs single-winner elections until the required number
/// of winners is reached or no remaining candidates are left.
pub fn run_election(builder: &Builder) -> Result<ElectionResult, VotingErrors> {
  let mut winners: Vec<String> = Vec::new();
  let mut remaining_candidates = builder._candidates.to_owned().unwrap_or_default();
  let mut all_round_stats: Vec<ElectionStats> = Vec::new();
  let mut spot_position = 0; // Track ranking spot

  while winners.len() < builder._rules.max_rankings_allowed.unwrap_or(usize::MAX as u32) as usize
    && !remaining_candidates.is_empty()
  {
    info!("Running election round with {} candidates", remaining_candidates.len());
    let election = run_voting_stats(&builder._votes, &builder._rules, &Some(remaining_candidates.clone()));
    match election {
      Ok(result) => {
        if let Some(mut elected_winners) = result.winners {
          info!("{}", format!("Elected winners: {:?}", elected_winners));
          winners.append(&mut elected_winners);
          remaining_candidates.retain(|c| !winners.contains(&c.name));
          spot_position += elected_winners.len() as u32;
          let election_stats = ElectionStats { spot_position, round_stats: result.round_stats };
          all_round_stats.push(election_stats);
        }
      }
      Err(error) => {
        error!("{}", format!("Election failed with error: {:?}", error));
      }
    };
  }
  info!("{}", "Election completed");
  Ok(ElectionResult { winners: Some(winners), stats: all_round_stats })
}

/// Runs an election (simple interface) using the instant-runoff voting
/// algorithm.
pub fn run_simple_election(votes: &[Vec<&str>], rules: &VoteRules) -> Result<ElectionResult, VotingErrors> {
  let mut builder = Builder::new(rules)?;
  let mut cand_set: BTreeSet<String> = BTreeSet::new();
  for ballot in votes.iter() {
    for choice in ballot.iter() {
      cand_set.insert(choice.to_string());
    }
  }
  let cand_vec: Vec<String> = cand_set.iter().cloned().collect();
  builder = builder.candidates(&cand_vec)?;
  for choices in votes.iter() {
    let cands: Vec<Vec<String>> = choices.iter().map(|c| vec![c.to_string()]).collect();
    builder.add_vote(&cands, 1)?;
  }
  run_election(&builder)
}

fn candidates_from_ballots(ballots: &[Ballot]) -> Vec<Candidate> {
  // Take everyone from the election as a valid candidate.
  let mut cand_set: BTreeSet<String> = BTreeSet::new();
  for ballot in ballots.iter() {
    for choice in ballot.candidates.iter() {
      if let BallotChoice::Candidate(name) = choice {
        cand_set.insert(name.clone());
      }
    }
  }
  let mut cand_vec: Vec<String> = cand_set.iter().cloned().collect();
  cand_vec.sort();
  cand_vec.iter().map(|n| Candidate { name: n.clone(), code: None, excluded: false }).collect()
}

/// Runs the voting algorithm with the given rules for the given votes.
///
/// Arguments:
/// * `coll` the collection of votes to process
/// * `rules` the rules that govern this election
/// * `candidates` the registered candidates for this election. If not provided,
///   the candidates will be inferred from the votes.
fn run_voting_stats(
  coll: &[Ballot],
  rules: &VoteRules,
  candidates_o: &Option<Vec<Candidate>>,
) -> Result<VotingResult, VotingErrors> {
  info!("Processing {} votes", coll.len());
  let candidates = candidates_o.to_owned().unwrap_or_else(|| candidates_from_ballots(coll));
  debug!("Candidates: {:?}", candidates);

  let cr: CheckResult = checks(coll, &candidates, rules)?;
  let checked_votes = cr.votes;
  debug!("Checked votes: {:?}, UWIs detected {:?}", checked_votes.len(), cr.count_exhausted_uwi_first_round);
  let all_candidates: Vec<(String, CandidateId)> = cr.candidates;
  {
    info!("Processing {:?} aggregated votes", checked_votes.len());
    let mut sorted_candidates: Vec<&(String, CandidateId)> = all_candidates.iter().collect();
    sorted_candidates.sort_by_key(|p| p.1);
    for p in sorted_candidates.iter() {
      info!("Candidate: {}: {}", p.1.0, p.0);
    }
  }

  let mut initial_count: VoteCount = VoteCount::EMPTY;
  for v in checked_votes.iter() {
    initial_count += v.count;
  }

  // We are done, stop here.
  let candidates_by_id: BTreeMap<CandidateId, String> =
    all_candidates.iter().map(|(cname, cid)| (*cid, cname.clone())).collect();

  // The candidates that are still running, in sorted order as defined by input.
  let mut cur_sorted_candidates: Vec<(String, CandidateId)> = all_candidates.clone();

  let mut cur_votes: Vec<VoteInternal> = checked_votes;
  let mut cur_stats: Vec<InternalRoundStatistics> = Vec::new();

  while cur_stats.iter().len() < 10000 {
    let round_id = (cur_stats.iter().len() + 1) as u32;
    debug!("run_voting_stats: Round id: {:?} cur_candidates: {:?}", round_id, cur_sorted_candidates);
    let has_initial_uwis =
      cur_stats.is_empty() && (!cr.uwi_first_votes.is_empty() || cr.count_exhausted_uwi_first_round > VoteCount::EMPTY);
    let round_res: RoundResult = if has_initial_uwis {
      // First round and we have some undeclared write ins.
      // Apply a special path to get rid of them.
      run_first_round_uwi(&cur_votes, &cr.uwi_first_votes, cr.count_exhausted_uwi_first_round, &cur_sorted_candidates)?
    } else {
      run_one_round(&cur_votes, rules, &cur_sorted_candidates, round_id)?
    };
    let round_stats = round_res.stats.clone();
    debug!("run_voting_stats: Round id: {:?} stats: {:?}", round_id, round_stats);
    print_round_stats(round_id, &round_stats, &all_candidates, round_res.vote_threshold);

    cur_votes = round_res.votes;
    cur_stats.push(round_res.stats);
    let stats = round_stats.candidate_stats;

    // Survivors are described in candidate order.
    let mut survivors: Vec<(String, CandidateId)> = Vec::new();
    for (s, cid) in cur_sorted_candidates.iter() {
      // Has this candidate been marked as eliminated? Skip it
      let is_eliminated =
        stats.iter().any(|(cid2, _, s)| matches!(s, RoundCandidateStatusInternal::Eliminated(_, _) if *cid == *cid2));
      if !is_eliminated {
        survivors.push((s.clone(), *cid));
      }
    }
    // Invariant: the number of candidates decreased or all the candidates are
    // winners
    let all_survivors_winners = stats.iter().all(|(_, _, s)| matches!(s, RoundCandidateStatusInternal::Elected));
    if !has_initial_uwis {
      assert!(
        all_survivors_winners || (survivors.len() < cur_sorted_candidates.len()),
        "The number of candidates did not decrease: {:?} -> {:?}",
        cur_sorted_candidates,
        survivors
      );
    }
    cur_sorted_candidates = survivors;
    // Check end. For now, simply check that we have a winner.
    assert!(!stats.is_empty());
    let winners: Vec<CandidateId> = stats
      .iter()
      .filter_map(|(cid, _, s)| match s {
        RoundCandidateStatusInternal::Elected => Some(*cid),
        _ => None,
      })
      .collect();
    if !winners.is_empty() {
      let stats = round_results_to_stats(&cur_stats, &candidates_by_id)?;
      let mut winner_names: Vec<String> = Vec::new();
      for cid in &winners {
        winner_names.push(candidates_by_id.get(cid).expect("Candidate not found").clone());
      }
      return Ok(VotingResult {
        threshold: round_res.vote_threshold.0,
        winners: Some(winner_names),
        round_stats: stats,
      });
    }
  }
  Err(VotingErrors::NoConvergence)
}

fn print_round_stats(
  round_id: RoundId,
  stats: &InternalRoundStatistics,
  candidate_names: &[(String, CandidateId)],
  vote_threshold: VoteCount,
) {
  info!("Round {} (winning threshold: {})", round_id, vote_threshold.0);
  let mut sorted_candidates = stats.candidate_stats.clone();
  sorted_candidates.sort_by_key(|(_, count, _)| -(count.0 as i64));
  let fetch_name = |cid: &CandidateId| candidate_names.iter().find(|(_, cid2)| cid2 == cid);
  for (cid, count, cstatus) in sorted_candidates.iter() {
    if let Some((name, _)) = fetch_name(cid) {
      let status = match cstatus {
        RoundCandidateStatusInternal::Elected => "elected".to_string(),
        RoundCandidateStatusInternal::StillRunning => "running".to_string(),
        RoundCandidateStatusInternal::Eliminated(transfers, exhausted) => {
          let mut s = String::from("eliminated:");
          if *exhausted > VoteCount::EMPTY {
            s.push_str(format!("{} exhausted, ", exhausted.0).as_str());
          }
          for (tcid, vc) in transfers {
            if let Some((tname, _)) = fetch_name(tcid) {
              s.push_str(format!("{} -> {}, ", vc.0, tname).as_str());
            }
          }
          s
        }
      };
      info!("{:7} {} -> {}", count.0, name, status);
    }
  }
  if let Some((transfers, exhausted)) = stats.uwi_elimination_stats.clone() {
    let mut s = String::from("undeclared candidates: ");
    if exhausted > VoteCount::EMPTY {
      s.push_str(format!("{} exhausted, ", exhausted.0).as_str());
    }
    for (tcid, vc) in transfers {
      if let Some((tname, _)) = fetch_name(&tcid) {
        s.push_str(format!("{} -> {}, ", vc.0, tname).as_str());
      }
    }
    info!("        {}", s);
  }
}

fn get_threshold(tally: &BTreeMap<CandidateId, VoteCount>) -> VoteCount {
  let total_count: VoteCount = tally.values().cloned().sum();
  if total_count == VoteCount::EMPTY {
    VoteCount::EMPTY
  } else {
    // let num_winners = num_winners as u64;
    // let threshold = (total_count.0 / (num_winners + 1)) + 1;
    // VoteCount(threshold)
    VoteCount((total_count.0 / 2) + 1)
  }
}

fn round_results_to_stats(
  results: &[InternalRoundStatistics],
  candidates_by_id: &BTreeMap<CandidateId, String>,
) -> Result<Vec<RoundStats>, VotingErrors> {
  let mut res: Vec<RoundStats> = Vec::new();
  for (idx, r) in results.iter().enumerate() {
    let round_id: RoundId = idx as u32 + 1;
    res.push(round_result_to_stat(r, round_id, candidates_by_id)?);
  }
  Ok(res)
}

fn round_result_to_stat(
  stats: &InternalRoundStatistics,
  round_id: RoundId,
  candidates_by_id: &BTreeMap<CandidateId, String>,
) -> Result<RoundStats, VotingErrors> {
  let mut rs = RoundStats {
    round: round_id,
    tally: Vec::new(),
    tally_results_elected: Vec::new(),
    tally_result_eliminated: Vec::new(),
  };

  for (cid, c, status) in stats.candidate_stats.iter() {
    let name: &String = candidates_by_id.get(cid).ok_or(VotingErrors::EmptyElection)?; // TODO: wrong error
    rs.tally.push((name.clone(), c.0));
    match status {
      RoundCandidateStatusInternal::StillRunning => {
        // Nothing to say about this candidate
      }
      RoundCandidateStatusInternal::Elected => {
        rs.tally_results_elected.push(name.clone());
      }
      RoundCandidateStatusInternal::Eliminated(transfers, exhausts)
        if (!transfers.is_empty()) || *exhausts > VoteCount::EMPTY =>
      {
        let mut pub_transfers: Vec<(String, u64)> = Vec::new();
        for (t_cid, t_count) in transfers {
          let t_name: &String = candidates_by_id.get(t_cid).ok_or(VotingErrors::EmptyElection)?; // TODO: wrong error
          pub_transfers.push((t_name.clone(), t_count.0));
        }
        rs.tally_result_eliminated.push(EliminationStats {
          name: name.clone(),
          transfers: pub_transfers,
          exhausted: exhausts.0,
        });
      }
      RoundCandidateStatusInternal::Eliminated(_, _) => {
        // Do not print a candidate if its corresponding stats are going to be
        // empty.
      }
    }
  }

  let uwi = "Undeclared Write-ins".to_string();

  if let Some((uwi_transfers, uwi_exhauster)) = stats.uwi_elimination_stats.clone() {
    let uwi_tally: VoteCount = uwi_transfers.iter().map(|(_, vc)| *vc).sum::<VoteCount>() + uwi_exhauster;
    if uwi_tally > VoteCount::EMPTY {
      rs.tally.push((uwi.clone(), uwi_tally.0));
    }
    let mut pub_transfers: Vec<(String, u64)> = Vec::new();
    for (t_cid, t_count) in uwi_transfers.iter() {
      let t_name: &String = candidates_by_id.get(t_cid).ok_or(VotingErrors::EmptyElection)?; // TODO: wrong error
      pub_transfers.push((t_name.clone(), t_count.0));
    }

    rs.tally_result_eliminated.push(EliminationStats {
      name: uwi,
      transfers: pub_transfers,
      exhausted: uwi_exhauster.0,
    });
  }

  rs.tally_result_eliminated.sort_by_key(|es| es.name.clone());
  rs.tally_results_elected.sort();
  Ok(rs)
}

fn run_first_round_uwi(
  votes: &[VoteInternal],
  uwi_first_votes: &[VoteInternal],
  uwi_first_exhausted: VoteCount,
  candidate_names: &[(String, CandidateId)],
) -> Result<RoundResult, VotingErrors> {
  let tally = compute_tally(votes, candidate_names);
  let mut elimination_stats: BTreeMap<CandidateId, VoteCount> = BTreeMap::new();
  for v in uwi_first_votes.iter() {
    let e = elimination_stats.entry(v.candidates.first_valid).or_insert(VoteCount::EMPTY);
    *e += v.count;
  }

  let full_stats = InternalRoundStatistics {
    candidate_stats: tally.iter().map(|(cid, vc)| (*cid, *vc, RoundCandidateStatusInternal::StillRunning)).collect(),
    uwi_elimination_stats: Some((elimination_stats.iter().map(|(cid, vc)| (*cid, *vc)).collect(), uwi_first_exhausted)),
  };

  let mut all_votes = votes.to_vec();
  all_votes.extend(uwi_first_votes.to_vec());

  Ok(RoundResult { votes: all_votes, stats: full_stats, vote_threshold: VoteCount::EMPTY })
}

fn compute_tally(
  votes: &[VoteInternal],
  candidate_names: &[(String, CandidateId)],
) -> BTreeMap<CandidateId, VoteCount> {
  let mut tally: BTreeMap<CandidateId, VoteCount> = BTreeMap::new();
  for (_, cid) in candidate_names.iter() {
    tally.insert(*cid, VoteCount::EMPTY);
  }
  for v in votes.iter() {
    if let Some(vc) = tally.get_mut(&v.candidates.first_valid) {
      *vc += v.count;
    }
  }
  tally
}

/// Returns the removed candidates, and the remaining votes
fn run_one_round(
  votes: &[VoteInternal],
  rules: &VoteRules,
  candidate_names: &[(String, CandidateId)],
  num_round: u32,
) -> Result<RoundResult, VotingErrors> {
  // Initialize the tally with the current candidate names to capture all the
  // candidates who do not even have a vote.
  let tally = compute_tally(votes, candidate_names);
  debug!("tally: {:?}", tally);

  let vote_threshold = get_threshold(&tally); // Update this line
  debug!("run_one_round: vote_threshold: {:?}", vote_threshold);

  // Only one candidate. It is the winner by any standard.
  // TODO: improve with multi candidate modes.
  if tally.len() == 1 {
    debug!("run_one_round: only one candidate, directly winning: {:?}", tally);
    let stats = InternalRoundStatistics {
      candidate_stats: tally.iter().map(|(cid, count)| (*cid, *count, RoundCandidateStatusInternal::Elected)).collect(),
      uwi_elimination_stats: Some((vec![], VoteCount::EMPTY)),
    };
    return Ok(RoundResult { votes: votes.to_vec(), stats, vote_threshold });
  }

  // Find the candidates to eliminate
  let p = find_eliminated_candidates(&tally, rules, candidate_names, num_round)?;
  let resolved_tiebreak: TiebreakSituation = p.1;
  let eliminated_candidates: BTreeSet<CandidateId> = p.0.iter().cloned().collect();

  // TODO strategy to pick the winning candidates

  if eliminated_candidates.is_empty() {
    return Err(VotingErrors::NoCandidateToEliminate);
  }
  debug!("run_one_round: tiebreak situation: {:?}", resolved_tiebreak);
  debug!("run_one_round: eliminated_candidates: {:?}", p.0);

  // Statistics about transfers:
  // For every eliminated candidates, keep the vote transfer, or the exhausted
  // vote.
  let mut elimination_stats: BTreeMap<CandidateId, (BTreeMap<CandidateId, VoteCount>, VoteCount)> =
    eliminated_candidates.iter().map(|cid| (*cid, (BTreeMap::new(), VoteCount::EMPTY))).collect();

  let remaining_candidates: BTreeSet<CandidateId> = candidate_names
    .iter()
    .filter_map(|p| match p {
      (_, cid) if !eliminated_candidates.contains(cid) => Some(*cid),
      _ => None,
    })
    .collect();

  // Filter the rest of the votes to simply keep the votes that still matter
  let rem_votes: Vec<VoteInternal> = votes
    .iter()
    .filter_map(|va| {
      // Remove the choices that are not valid anymore and collect statistics.
      let new_rank = va.candidates.filtered_candidate(
        &remaining_candidates,
        rules.duplicate_candidate_mode,
        rules.overvote_rule,
        rules.max_skipped_rank_allowed,
      );
      let old_first = va.candidates.first_valid;
      let new_first = new_rank.clone().map(|nr| nr.first_valid);

      match new_first {
        None => {
          // Ballot is now exhausted. Record the exhausted vote.
          let e = elimination_stats.entry(old_first).or_insert((BTreeMap::new(), VoteCount::EMPTY));
          e.1 += va.count;
        }
        Some(new_first_cid) if new_first_cid != old_first => {
          // The ballot has been transfered. Record the transfer.
          let e = elimination_stats.entry(old_first).or_insert((BTreeMap::new(), VoteCount::EMPTY));
          let e2 = e.0.entry(new_first_cid).or_insert(VoteCount::EMPTY);
          *e2 += va.count;
        }
        _ => {
          // Nothing to do, the first choice is the same.
        }
      }

      new_rank.map(|rc| VoteInternal { candidates: rc, count: va.count })
    })
    .collect();

  // Check if some candidates are winners.
  // Right now, it is simply if one candidate is left.
  let remainers: BTreeMap<CandidateId, VoteCount> = tally
    .iter()
    .filter_map(|(cid, vc)| if eliminated_candidates.contains(cid) { None } else { Some((*cid, *vc)) })
    .collect();

  debug!("run_one_round: remainers: {:?}", remainers);
  let mut winners: BTreeSet<CandidateId> = BTreeSet::new();
  // If a tiebreak was resolved in this round, do not select a winner.
  // This is just an artifact of the reference implementation.
  if resolved_tiebreak == TiebreakSituation::Clean {
    for (&cid, &count) in remainers.iter() {
      if count >= vote_threshold {
        debug!("run_one_round: {:?} has count {:?}, marking as winner", cid, count);
        winners.insert(cid);
      }
    }
  }

  let mut candidate_stats: Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)> = Vec::new();
  for (&cid, &count) in tally.iter() {
    if let Some((transfers, exhaust)) = elimination_stats.get(&cid) {
      candidate_stats.push((
        cid,
        count,
        RoundCandidateStatusInternal::Eliminated(transfers.iter().map(|(cid2, c2)| (*cid2, *c2)).collect(), *exhaust),
      ))
    } else if winners.contains(&cid) {
      candidate_stats.push((cid, count, RoundCandidateStatusInternal::Elected));
    } else {
      // Not eliminated, still running
      candidate_stats.push((cid, count, RoundCandidateStatusInternal::StillRunning));
    }
  }

  Ok(RoundResult {
    votes: rem_votes,
    stats: InternalRoundStatistics { candidate_stats, uwi_elimination_stats: None },
    vote_threshold,
  })
}

fn find_eliminated_candidates(
  tally: &BTreeMap<CandidateId, VoteCount>,
  rules: &VoteRules,
  candidate_names: &[(String, CandidateId)],
  num_round: u32,
) -> Result<(Vec<CandidateId>, TiebreakSituation), VotingErrors> {
  println!("tally?: {:?} - round {:?}", tally, num_round);
  // Try to eliminate candidates in batch
  if rules.elimination_algorithm == EliminationAlgorithm::Batch {
    if let Some(v) = find_eliminated_candidates_batch(tally) {
      return Ok((v, TiebreakSituation::Clean));
    }
  }

  if let Some((v, tb)) = find_eliminated_candidates_single(tally, rules.tiebreak_mode, candidate_names, num_round) {
    return Ok((v, tb));
  }
  // No candidate to eliminate.
  Err(VotingErrors::EmptyElection)
}

fn find_eliminated_candidates_batch(tally: &BTreeMap<CandidateId, VoteCount>) -> Option<Vec<CandidateId>> {
  // Sort the candidates in increasing tally.
  let mut sorted_tally: Vec<(CandidateId, VoteCount)> = tally.iter().map(|(&cid, &vc)| (cid, vc)).collect();
  sorted_tally.sort_by_key(|(_, vc)| *vc);

  // the vote count for this candidate and the cumulative count (excluding the
  // current one)
  let mut sorted_tally_cum: Vec<(CandidateId, VoteCount, VoteCount)> = Vec::new();
  let mut curr_count = VoteCount::EMPTY;
  for (cid, cur_vc) in sorted_tally.iter() {
    sorted_tally_cum.push((*cid, *cur_vc, curr_count));
    curr_count += *cur_vc;
  }
  debug!("find_eliminated_candidates_batch: sorted_tally_cum: {:?}", sorted_tally_cum);

  // Find the largest index for which the previous cumulative count is strictly
  // lower than the current vote count. Anything below will not be able to
  // transfer higher.

  let large_gap_idx = sorted_tally_cum
    .iter()
    .enumerate()
    .filter(|(_, (_, cur_vc, previous_cum_count))| previous_cum_count < cur_vc)
    .last();

  // The idx == 0 element is not relevant because the previous cumulative count
  // was zero.
  if let Some((idx, _)) = large_gap_idx {
    if idx > 0 {
      let res = sorted_tally.iter().map(|(cid, _)| *cid).take(idx).collect();
      debug!("find_eliminated_candidates_batch: found a batch to eliminate: {:?}", res);
      return Some(res);
    }
  }
  debug!("find_eliminated_candidates_batch: no candidates to eliminate");
  None
}

// Flag to indicate if a tiebreak happened.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
enum TiebreakSituation {
  Clean,           // Did not happen
  TiebreakOccured, // Happened and had to be resolved.
}

// Elimination method for single candidates.
fn find_eliminated_candidates_single(
  tally: &BTreeMap<CandidateId, VoteCount>,
  tiebreak: TieBreakMode,
  candidate_names: &[(String, CandidateId)],
  num_round: u32,
) -> Option<(Vec<CandidateId>, TiebreakSituation)> {
  // TODO should be a programming error
  if tally.is_empty() {
    return None;
  }

  // Only one candidate left, it is the winner by default.
  // No need to eliminate candidates.
  if tally.len() == 1 {
    debug!("find_eliminated_candidates_single: Only one candidate left in tally, no one to eliminate: {:?}", tally);
    return None;
  }

  assert!(tally.len() >= 2);

  let min_count: VoteCount = *tally.values().min().expect("No votes found");

  let all_smallest: Vec<CandidateId> =
    tally.iter().filter_map(|(cid, vc)| if *vc <= min_count { Some(cid) } else { None }).cloned().collect();
  println!("find_eliminated_candidates_single: all_smallest: {:?}", all_smallest);

  debug!("find_eliminated_candidates_single: all_smallest: {:?}", all_smallest);
  assert!(!all_smallest.is_empty());

  // No tiebreak, the logic below is not relevant.
  if all_smallest.len() == 1 {
    return Some((all_smallest, TiebreakSituation::Clean));
  }

  // Look at the tiebreak mode:
  let mut sorted_candidates: Vec<CandidateId> = match tiebreak {
    TieBreakMode::UseCandidateOrder => {
      let candidate_order: BTreeMap<CandidateId, usize> =
        candidate_names.iter().enumerate().map(|(idx, (_, cid))| (*cid, idx)).collect();
      let mut res = all_smallest;
      res.sort_by_key(|cid| candidate_order.get(cid).expect("Candidate not found in order"));
      // For loser selection, the selection is done in reverse order according to the
      // reference implementation.
      res.reverse();
      debug!(
        "find_eliminated_candidates_single: sorted candidates in elimination queue using tiebreak mode usecandidateorder: {:?}",
        res
      );
      res
    }
    TieBreakMode::Random(seed) => {
      let cand_with_names: Vec<(CandidateId, String)> = all_smallest
        .iter()
        .map(|cid| {
          let m: Option<(CandidateId, String)> = candidate_names
            .iter()
            .filter_map(|(n, cid2)| if cid == cid2 { Some((*cid2, n.clone())) } else { None })
            .next();
          m.expect("Option is None when unwrap() was called")
        })
        .collect();
      let res = candidate_permutation_crypto(&cand_with_names, seed, num_round);
      debug!(
        "find_eliminated_candidates_single: sorted candidates in elimination queue using tiebreak mode random: {:?}",
        res
      );
      res
    }
  };

  // Temp copy
  let sc = sorted_candidates.clone();

  // TODO check that it is accurate to do.
  // For now, just select a single candidate for removal.
  sorted_candidates.truncate(1);

  // We are currently proceeding to remove all the candidates. Do not remove the
  // last one.
  if sc.len() == tally.len() {
    let last = sc.last().expect("No elements in collection");
    sorted_candidates.retain(|cid| cid != last);
  }
  Some((sorted_candidates, TiebreakSituation::TiebreakOccured))
}

// All the failure modes when trying to read the next element in a ballot
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
enum AdvanceRuleCheck {
  DuplicateCandidates,
  FailOvervote,
  FailSkippedRank,
}

// True if the rules are respected
fn check_advance_rules(
  initial_slice: &[Choice],
  duplicate_policy: DuplicateCandidateMode,
  overvote: OverVoteRule,
  skipped_ranks: MaxSkippedRank,
) -> Option<AdvanceRuleCheck> {
  if duplicate_policy == DuplicateCandidateMode::Exhaust {
    let mut seen_cids: BTreeSet<CandidateId> = BTreeSet::new();
    for choice in initial_slice.iter() {
      match *choice {
        Choice::Filled(cid) if seen_cids.contains(&cid) => {
          return Some(AdvanceRuleCheck::DuplicateCandidates);
        }
        Choice::Filled(cid) => {
          seen_cids.insert(cid);
        }
        _ => {}
      }
    }
  }

  // Overvote rule
  let has_initial_overvote = initial_slice.iter().any(|c| *c == Choice::Overvote);
  if has_initial_overvote && overvote == OverVoteRule::ExhaustImmediately {
    debug!("advance_voting: has initial overvote and exhausting {:?}", initial_slice);
    return Some(AdvanceRuleCheck::FailOvervote);
  }

  // Skipped rank rule
  if skipped_ranks == MaxSkippedRank::ExhaustOnFirstOccurence {
    let has_skippable_elements = initial_slice.iter().any(|choice| matches!(choice, Choice::BlankOrUndervote));
    if has_skippable_elements {
      debug!("advance_voting:exhaust on first blank occurence: {:?}", initial_slice);
      return Some(AdvanceRuleCheck::FailSkippedRank);
    }
  }

  if let MaxSkippedRank::MaxAllowed(range_len) = skipped_ranks {
    let mut start_skipped_block: Option<usize> = None;
    let rl = range_len as usize;
    for (idx, choice) in initial_slice.iter().enumerate() {
      match (choice, start_skipped_block) {
        // We went beyond the threshold
        (Choice::BlankOrUndervote, Some(start_idx)) if idx >= start_idx + rl => {
          debug!("advance_voting:exhaust on multiple occurence: {:?}", initial_slice);
          return Some(AdvanceRuleCheck::FailSkippedRank);
        }
        // We are starting a new block
        (Choice::BlankOrUndervote, None) => {
          start_skipped_block = Some(idx);
        }
        // We are exiting a block or encountering a new element. Reset.
        _ => {
          start_skipped_block = None;
        }
      }
    }
  }

  None
}

// The algorithm is lazy. It will only apply the rules up to finding the next
// candidate. Returned bool indicades if UWI's were encountered in the move to
// the first valid candidate. TODO: this function slightly deviates for the
// reference implementation in the following case: (blanks 1) Undeclared (blanks
// 2) Filled(_) ... The reference implementation will only validate up to
// Undeclared and this function will validate up to Filled.
// In practice, this will cause the current implementation to immediately
// discard a ballot, while the reference implementation first assigns the ballot
// to UWI and then exhausts it.
fn advance_voting(
  choices: &[Choice],
  still_valid: &BTreeSet<CandidateId>,
  duplicate_policy: DuplicateCandidateMode,
  overvote: OverVoteRule,
  skipped_ranks: MaxSkippedRank,
) -> Option<(CandidateId, Vec<Choice>)> {
  // Find a potential candidate.
  let first_candidate = choices.iter().enumerate().find_map(|(idx, choice)| match choice {
    Choice::Filled(cid) if still_valid.contains(cid) => Some((idx, cid)),
    _ => None,
  });
  if let Some((idx, cid)) = first_candidate {
    // A valid candidate was found, but still look in the initial slice to find if
    // some overvote or multiple blanks occured.
    let initial_slice = &choices[.. idx];

    if check_advance_rules(initial_slice, duplicate_policy, overvote, skipped_ranks).is_some() {
      return None;
    }

    let final_slice = &choices[idx + 1 ..];
    Some((*cid, final_slice.to_vec()))
  } else {
    None
  }
}

// For the 1st round, the initial choice may also be undeclared.
fn advance_voting_initial(
  choices: &[Choice],
  still_valid: &BTreeSet<CandidateId>,
  duplicate_policy: DuplicateCandidateMode,
  overvote: OverVoteRule,
  skipped_ranks: MaxSkippedRank,
) -> Option<Vec<Choice>> {
  // Find a potential candidate.
  let first_candidate: Option<usize> = choices.iter().enumerate().find_map(|(idx, choice)| match choice {
    Choice::Filled(cid) if still_valid.contains(cid) => Some(idx),
    Choice::Undeclared => Some(idx),
    _ => None,
  });
  if let Some(idx) = first_candidate {
    // A valid candidate was found, but still look in the initial slice to find if
    // some overvote or multiple blanks occured.
    let initial_slice = &choices[.. idx];

    if check_advance_rules(initial_slice, duplicate_policy, overvote, skipped_ranks).is_some() {
      return None;
    }

    // This final slice includes the pivot element.
    let final_slice = &choices[idx ..];
    Some(final_slice.to_vec())
  } else {
    None
  }
}

struct CheckResult {
  votes: Vec<VoteInternal>,
  // further_votes: Vec<VoteInternal>,
  candidates: Vec<(String, CandidateId)>,
  uwi_first_votes: Vec<VoteInternal>,
  count_exhausted_uwi_first_round: VoteCount,
}

// Candidates are returned in the same order.
fn checks(coll: &[Ballot], reg_candidates: &[Candidate], rules: &VoteRules) -> Result<CheckResult, VotingErrors> {
  debug!("checks: coll size: {:?}", coll.len());
  let blacklisted_candidates: BTreeSet<String> =
    reg_candidates.iter().filter_map(|c| if c.excluded { Some(c.name.clone()) } else { None }).collect();
  let candidates: BTreeMap<String, CandidateId> =
    reg_candidates.iter().enumerate().map(|(idx, c)| (c.name.clone(), CandidateId((idx + 1) as u32))).collect();

  let valid_cids: BTreeSet<CandidateId> = candidates.values().cloned().collect();

  // The votes that are validated and that have a candidate from the first round
  let mut validated_votes: Vec<VoteInternal> = vec![];
  // The votes that are valid but do not have a candidate in the first round.
  let mut uwi_validated_votes: Vec<VoteInternal> = vec![];
  // The count of votes that are immediately exhausted with a UWI in the first
  // round.
  let mut uwi_exhausted_first_round: VoteCount = VoteCount::EMPTY;

  for v in coll.iter() {
    let mut choices: Vec<Choice> = vec![];
    for c in v.candidates.iter() {
      let choice: Choice = match c {
        BallotChoice::Candidate(name) if blacklisted_candidates.contains(name) => {
          unimplemented!("blacklisted not implemented");
        }
        BallotChoice::Candidate(name) => {
          if let Some(cid) = candidates.get(name) {
            Choice::Filled(*cid)
          } else {
            // Undeclared candidate
            Choice::Undeclared
          }
        }
        BallotChoice::Blank => Choice::BlankOrUndervote,
        BallotChoice::Undervote => Choice::BlankOrUndervote,
        BallotChoice::Overvote => Choice::Overvote,
        BallotChoice::UndeclaredWriteIn => Choice::Undeclared,
      };
      choices.push(choice);
    }

    let count = VoteCount(v.count);
    // The first choice is a valid one. A ballot can be constructed out of it.

    let initial_advance_opt = advance_voting_initial(
      &choices,
      &valid_cids,
      rules.duplicate_candidate_mode,
      rules.overvote_rule,
      rules.max_skipped_rank_allowed,
    );

    if let Some(initial_advance) = initial_advance_opt {
      // Check the head of the ballot.
      if let Some(Choice::Filled(cid)) = initial_advance.first() {
        let candidates = RankedVoteCandidates { first_valid: *cid, rest: initial_advance[1 ..].to_vec() };
        validated_votes.push(VoteInternal { candidates, count });
      } else if let Some(Choice::Undeclared) = initial_advance.first() {
        // Valid and first choice is undeclared. See if the rest is a valid vote.
        if let Some((first_cid, rest)) = advance_voting(
          &initial_advance,
          &valid_cids,
          rules.duplicate_candidate_mode,
          rules.overvote_rule,
          rules.max_skipped_rank_allowed,
        ) {
          // The vote is still valid by advancing, we keep it
          let candidates = RankedVoteCandidates { first_valid: first_cid, rest };
          uwi_validated_votes.push(VoteInternal { candidates, count });
        } else {
          // The vote was valid up to undeclared but not valid anymore after it.
          // Exhaust immediately.
          uwi_exhausted_first_round += count;
        }
      } else {
        panic!("checks: Should not reach this branch:choices: {:?} initial_advance: {:?}", choices, initial_advance);
      }
    } else {
      // Vote is being discarded, nothing to read in it with the given rules.
    }
  }

  debug!("checks: vote aggs size: {:?}  candidates: {:?}", validated_votes.len(), candidates.len());

  let ordered_candidates: Vec<(String, CandidateId)> =
    reg_candidates.iter().filter_map(|c| candidates.get(&c.name).map(|cid| (c.name.clone(), *cid))).collect();

  debug!("checks: ordered_candidates {:?}", ordered_candidates);
  Ok(CheckResult {
    votes: validated_votes,
    uwi_first_votes: uwi_validated_votes,
    candidates: ordered_candidates,
    count_exhausted_uwi_first_round: uwi_exhausted_first_round,
  })
}

/// Generates a "random" permutation of the candidates. Random in this context
/// means hard to guess in advance. This uses a cryptographic algorithm that is
/// resilient to collisions.
fn candidate_permutation_crypto(candidates: &[(CandidateId, String)], seed: u32, num_round: u32) -> Vec<CandidateId> {
  let mut data: Vec<(CandidateId, String)> =
    candidates.iter().map(|(cid, name)| (*cid, format!("{:08}{:08}{}", seed, num_round, name))).collect();
  data.sort_by_key(|p| p.1.clone());
  data.iter().map(|p| p.0).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ranked_vote_decode_memo() {
    let mut vote = RankedVote::new("1", "1", "", 100, BlockStatus::Pending, 100, 1);
    vote.update_memo("E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi");
    assert_eq!(vote.decode_memo().unwrap(), "MEF 1 3 1 39");
  }

  #[test]
  fn test_parse_decoded_ranked_votes_memo() {
    let round_id = "1";
    let proposal_ids: Vec<&str> = vec!["3", "1", "39"];
    let mut votes = get_test_votes();

    let (_round_id, _proposal_ids) = votes[0].parse_decoded_ranked_votes_memo(round_id).unwrap();
    assert_eq!(_round_id, round_id);
    assert_eq!(_proposal_ids, proposal_ids);
  }

  #[test]
  fn test_process_ranked_votes() {
    let votes = get_test_votes();
    let binding = Wrapper(votes).process_ranked_vote(1, 129);
    let processed: Vec<RankedVote> = binding.0.values().cloned().collect();

    assert_eq!(processed.len(), 10);

    let a1 = processed.iter().find(|s| s.account == "1").unwrap();
    let a2 = processed.iter().find(|s| s.account == "2").unwrap();

    assert_eq!(a1.account, "1");
    assert_eq!(a1.hash, "1");
    assert_eq!(a1.memo, "Votes: [\"3\", \"1\", \"39\"]");
    assert_eq!(a1.height, 331718);
    assert_eq!(a1.status, BlockStatus::Canonical);
    assert_eq!(a1.nonce, 1);

    assert_eq!(a2.account, "2");
    assert_eq!(a2.hash, "2");
    assert_eq!(a2.memo, "Votes: [\"3\", \"1\", \"39\"]");
    assert_eq!(a2.height, 341719);
    assert_eq!(a2.status, BlockStatus::Pending);
    assert_eq!(a2.nonce, 2);
  }

  #[test]
  fn test_ranked_vote_update_memo() {
    let mut vote = RankedVote::new("1", "1", "Initial memo", 100, BlockStatus::Pending, 100, 1);
    vote.update_memo("Updated memo");
    assert_eq!(vote.memo, "Updated memo");
  }

  #[test]
  fn test_ranked_vote_update_status() {
    let mut vote = RankedVote::new("1", "1", "Memo", 100, BlockStatus::Pending, 100, 1);
    vote.update_status(BlockStatus::Canonical);
    assert_eq!(vote.status, BlockStatus::Canonical);
  }

  #[test]
  fn test_ranked_vote_is_newer_than() {
    let vote1 = RankedVote::new("1", "1", "Memo", 100, BlockStatus::Pending, 100, 1);
    let vote2 = RankedVote::new("1", "1", "Memo", 101, BlockStatus::Pending, 100, 1);
    assert!(vote2.is_newer_than(&vote1));
  }

  #[test]
  fn test_run_election() {
    let votes = vec![
      vec!["39", "5", "2", "1", "4", "3"],
      vec!["39", "3", "2", "4", "1"],
      vec!["1", "3", "39", "5", "2", "4"],
      vec!["1", "39", "5", "2", "3", "4"],
      vec!["2", "1", "39", "3", "4", "5"],
      vec!["3", "4", "39", "5", "2", "1"],
      vec!["1", "3", "39", "5", "2", "4"],
      vec!["5", "2", "39", "4", "3", "5"],
    ];

    let rules = VoteRules::default();

    let result = run_simple_election(&votes, &rules).unwrap();
    assert_eq!(result.winners.unwrap(), vec!["1", "39", "3", "5", "2", "4"]);
  }
  #[test]
  fn test_run_election_eleven_votes() {
    let votes = vec![
      vec!["5", "4", "1", "39"],
      vec!["4", "2"],
      vec!["3", "4", "39", "5", "2", "1"],
      vec!["1", "2", "3", "4"],
      vec!["1", "3", "39", "5", "2", "4"],
      vec!["1", "39", "3", "5", "2", "4"],
      vec!["2", "1", "39", "3", "4", "5"],
      vec!["5", "39", "3", "2", "4", "1"],
      vec!["39", "5", "2", "1", "4", "3"],
      vec!["4", "2"],
      vec!["3"],
    ];

    let rules = VoteRules::default();

    let result = run_simple_election(&votes, &rules).unwrap();
    assert_eq!(result.winners.unwrap(), vec!["1", "39", "2", "3", "4", "5"]);
  }

  #[test]
  fn test_run_election_two_votes() {
    let votes = vec![vec!["2", "4", "1", "3"], vec!["3", "1", "39"]];
    let rules = VoteRules::default();

    let result = run_simple_election(&votes, &rules).unwrap();
    assert_eq!(result.winners.unwrap(), vec!["2", "3", "1", "39", "4"]);
  }

  #[test]
  fn test_run_election_three_votes() {
    let votes = vec![vec!["2", "4", "1", "3"], vec!["3", "1", "39"], vec!["5", "4", "1", "39"]];

    let rules = VoteRules::default();

    let result = run_simple_election(&votes, &rules).unwrap();
    assert_eq!(result.winners.unwrap(), vec!["2", "4", "1", "3", "39", "5"]);
  }

  fn get_test_votes() -> Vec<RankedVote> {
    vec![
      RankedVote::new(
        "1",
        "1",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        331718,
        BlockStatus::Canonical,
        1730897878000,
        1,
      ),
      RankedVote::new(
        "2",
        "2",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        341719,
        BlockStatus::Pending,
        1730897878000,
        2,
      ),
      RankedVote::new(
        "3",
        "3",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        351320,
        BlockStatus::Pending,
        1730897878000,
        3,
      ),
      RankedVote::new(
        "4",
        "4",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        352721,
        BlockStatus::Pending,
        1730897878000,
        4,
      ),
      RankedVote::new(
        "5",
        "5",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        353722,
        BlockStatus::Pending,
        1730897878000,
        5,
      ),
      RankedVote::new(
        "6",
        "6",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        354723,
        BlockStatus::Pending,
        1730897878000,
        6,
      ),
      RankedVote::new(
        "7",
        "7",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        355724,
        BlockStatus::Pending,
        1730897878000,
        7,
      ),
      RankedVote::new(
        "8",
        "8",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        356725,
        BlockStatus::Pending,
        1730897878000,
        8,
      ),
      RankedVote::new(
        "9",
        "9",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        357726,
        BlockStatus::Pending,
        1730897878000,
        9,
      ),
      RankedVote::new(
        "10",
        "10",
        "E4YkwtLx9t8gRCWWoc8cACHxKFSywt23uaGKTfmkwF1sNMh87FMEi",
        358727,
        BlockStatus::Pending,
        1730897878000,
        10,
      ),
      RankedVote::new(
        "11",
        "11",
        "E4Yf7epFtpM8YAsxcGVagQQKmtUpwj8nKTWMQnWbXyhg7hE6ceJhJ",
        358728,
        BlockStatus::Pending,
        1730897878000,
        11,
      ),
    ]
  }
}
