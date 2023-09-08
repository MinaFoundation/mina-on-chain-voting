import { Brightness4, Brightness7 } from '@mui/icons-material';
import { Box, Container, IconButton, Link, Stack, Typography } from '@mui/material';

import { useTheme } from 'components/provider';

export type PropsLayoutProps = React.PropsWithChildren & {
  title?: string;
};

export const PageLayout = (props: PropsLayoutProps) => {
  const { theme, setTheme } = useTheme();

  return (
    <Stack height="100vh" width="100%" display="flex" alignItems="center" gap={3}>
      {/* Header */}
      <Stack
        px={5}
        py={1.75}
        width="100%"
        direction="row"
        justifyContent="space-between"
        alignItems="center"
        borderBottom={0.2}
        borderColor="hsl(0, 0%, 24.3%)"
      >
        <Typography variant="h5" fontWeight={500} sx={{ cursor: 'default' }}>
          {props.title || 'On-Chain Voting'}
        </Typography>

        <Box>
          <IconButton
            onClick={() => setTheme(theme.key === 'dark' ? 'light' : 'dark')}
            color="inherit"
            aria-label="theme-toggle"
          >
            {theme.key === 'dark' ? <Brightness7 /> : <Brightness4 />}
          </IconButton>
        </Box>
      </Stack>

      {/* Body */}
      <Container
        maxWidth="xl"
        sx={{
          mb: 'auto',
          '&.MuiContainer-maxWidthXl': {
            maxWidth: '110rem',
          },
        }}
      >
        <Stack spacing={1}>{props.children}</Stack>
      </Container>

      {/* Footer */}
      <Stack
        py={1.25}
        width="100%"
        direction="row"
        justifyContent="center"
        alignItems="center"
        borderTop={0.2}
        borderColor="hsl(0, 0%, 24.3%)"
      >
        <Stack justifyContent="center" alignItems="center">
          <Link href="https://github.com/Granola-Team/mina-on-chain-voting" color="#FF6739">
            <Typography variant="body2">GitHub</Typography>
          </Link>
          <Link href="https://granola.team" color="#FF6739">
            <Typography variant="caption">Made with ❤️ by Granola</Typography>
          </Link>
        </Stack>
      </Stack>
    </Stack>
  );
};
