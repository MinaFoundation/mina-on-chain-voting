import { type ThemeOptions, createTheme } from '@mui/material/styles';
import { deepmerge } from '@mui/utils';

import { BaseTheme } from '../BaseTheme';

/**
 * The dark theme.
 */
const _DarkTheme: ThemeOptions = {
  palette: {
    mode: 'dark',
    background: {
      default: '#232323',
    },
  },
};

export const DarkTheme = createTheme(deepmerge(BaseTheme, _DarkTheme));
