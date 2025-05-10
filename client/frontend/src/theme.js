// src/theme.js
import { createTheme } from "@mui/material/styles";

const theme = createTheme({
    palette: {
        mode: 'dark', // dark mode for richer feeling
        primary: {
            main: '#1e3a8a',  // Tailwind's blue-800
        },
        secondary: {
            main: '#64748b',  // Tailwind's slate-500
        },
        background: {
            default: '#0f172a',  // Dark slate
            paper: '#1e293b',    // Slightly lighter
        },
    },
    shape: {
        borderRadius: 12, // Softer corners globally
    },
    typography: {
        fontFamily: 'Inter, sans-serif', // Modern sans-serif font
    },
});

export default theme;
