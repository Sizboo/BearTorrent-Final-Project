// src/index.jsx

import React from 'react';
import ReactDOM from 'react-dom/client';
import './tailwind.output.css'; // Your Tailwind styles
import App from './App';
import { ThemeProvider } from "@mui/material/styles";
import theme from './theme';
import reportWebVitals from './reportWebVitals'; // If you track performance

const root = ReactDOM.createRoot(document.getElementById('root'));

root.render(
    <React.StrictMode>
        <ThemeProvider theme={theme}>
            <App />
        </ThemeProvider>
    </React.StrictMode>
);

// Optional: for analytics / performance tracking
reportWebVitals();
