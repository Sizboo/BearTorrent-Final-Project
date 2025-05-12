import React from 'react';
import ReactDOM from 'react-dom/client';
import './tailwind.output.css';
import AppRoutes from './AppRoutes';
import { ThemeProvider } from "@mui/material/styles";
import theme from './theme';
import reportWebVitals from './reportWebVitals';


// Prevent browser from hijacking drop events so Tauri can receive them
["dragenter", "dragover", "dragleave", "drop"].forEach(eventName => {
    window.addEventListener(eventName, e => {
        e.preventDefault();
        e.stopPropagation();
    }, false);
});

const root = ReactDOM.createRoot(document.getElementById('root'));

root.render(
    <React.StrictMode>
        <ThemeProvider theme={theme}>
            <AppRoutes /> {/* ✅ AppRoutes now handles routing via HashRouter */}
        </ThemeProvider>
    </React.StrictMode>
);

reportWebVitals();
