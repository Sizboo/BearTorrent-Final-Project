import React from 'react';
import { Routes, Route, HashRouter } from 'react-router-dom'; // ✅ Clean single import
import App from './pages/App';        // Home page
import Files from './pages/Files';    // Files page

export default function AppRoutes() {
    return (
        <HashRouter>
            <Routes>
                <Route path="/" element={<App />} />
                <Route path="/files" element={<Files />} />
            </Routes>
        </HashRouter>
    );
}
