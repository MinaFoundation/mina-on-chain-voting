import { PrivateKey, Mina, AccountUpdate, UInt64 } from "o1js";

let [, , network, vote, skRaw] = process.argv;
if (
  network &&
  !!!(
    {
      devnet: true,
      mainnet: true,
    } as Record<string, boolean>
  )[network]
) {
  throw new Error(
    'Expected either `"devnet"` or `"mainnet"` as first argument'
  );
}

const voteExample = '`"MIP3"` or `"no MIP3"`';
if (!vote) {
  throw new Error(
    `Expected vote as second argument (for instance: ${voteExample})`
  );
}
if (!(vote.startsWith("MIP") || vote.startsWith("no MIP"))) {
  throw new Error(
    `Malformed vote argument. Expected shape such as ${voteExample}`
  );
}
const proposalNumber = vote.split("MIP").pop();
if (
  !proposalNumber ||
  (() => {
    try {
      parseInt(proposalNumber);
      return false;
    } catch (_e) {
      return true;
    }
  })()
) {
  throw new Error(
    `Malformed vote argument. Expected after \`"MIP"\`. For instance: {voteExample}`
  );
}
if (!skRaw) {
  throw new Error(
    "Must supply private key as 3rd argument in order to sign the vote transaction"
  );
}

const sk = PrivateKey.fromBase58(skRaw);
const pk = sk.toPublicKey();

Mina.setActiveInstance(
  Mina.Network(`https://${network}.minaprotocol.network/graphql`)
);

await Mina.transaction({ fee: 1, memo: vote, sender: pk }, async function () {
  const au = AccountUpdate.create(pk);
  au.send({ to: pk, amount: 1 });
})
  .sign([sk])
  .prove()
  .send()
  .wait();
