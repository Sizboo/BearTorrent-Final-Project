import { useEffect, useState } from "react";
import React from 'react';
import { useNavigate } from 'react-router-dom';
import { AnimatePresence, motion } from "framer-motion";
import { invoke } from '@tauri-apps/api/core';
import FileTable from "../components/FileTable";
import FileDetailSidebar from "../components/FileDetailSidebar";
import ToggleButton from '../components/ToggleButton';
import BrowseFileButton from '../components/BrowseFileButton';
import "../index.css";
import "../shimmer.css";
import '../modern-styles.css';

console.log("Is Tauri environment:", "__TAURI_IPC__" in window);


const initialFiles = [
    { name: "report.pdf", size: 1.2, type: "PDF", lastModified: "2023-09-12" },
    { name: "presentation.pptx", size: 4.5, type: "Presentation", lastModified: "2023-09-10" },
    { name: "photo.jpg", size: 2.1, type: "Image", lastModified: "2023-09-14" },
    { name: "notes.txt", size: 0.5, type: "Text", lastModified: "2023-09-05" },
];

const SidebarMotion = motion.create("div");



export default function Files() {
    const navigate = useNavigate();
    const [message, setMessage] = useState("Click to say hello...");
    const [files, setFiles] = useState([]);
    const [isToggled, setIsToggled] = useState(false);






    function handleUploadClick() {
        handleClick();
        handleGreetings();
    }


    const handleClick = async () => {
        console.log("Welcome to the Files");
        setMessage("Waiting for Rust...");
        const result = await invoke("say_hello_delayed");
        console.log("[JS] Got result from Rust:", result);
        setMessage(result);
    };



    const [selected, setSelected] = useState(null);
    const [sortField, setSortField] = useState("name");
    const [sortAsc, setSortAsc] = useState(true);

    const sortedFiles = [...initialFiles].sort((a, b) => {
        const valA = a[sortField];
        const valB = b[sortField];
        if (typeof valA === "string") return sortAsc ? valA.localeCompare(valB) : valB.localeCompare(valA);
        return sortAsc ? valA - valB : valB - valA;
    });

    return (


        <div className="flex flex-col h-screen bg-gradient-to-br from-slate-700 via-slate-600 to-slate-500 text-white font-sans">
            {/* Full-width Header */}
            <header className="bg-gradient-to-r from-blue-800 to-blue-600 shadow-xl text-white p-4 flex justify-between items-center w-full">
                <h1 className="text-2xl font-bold tracking-wide flex items-center gap-2">
                    📁 <span>File Manager</span>
                </h1>
                <nav className="flex gap-6">
                    <button className="menu-button rounded-lg px-4 py-2 hover:bg-blue-700 transition-all duration-150" onClick={() => navigate("/")}>Home</button>
                    <button className="menu-button rounded-lg px-4 py-2 hover:bg-blue-700 transition-all duration-150"onClick={handleUploadClick}>Upload</button>
                    <button className="menu-button rounded-lg px-4 py-2 hover:bg-blue-700 transition-all duration-150"onClick={() => navigate("/files")}>Files</button>
                    <ToggleButton />
                </nav>
            </header>

            {/* Main Content Layout */}
            <div className="p-6 text-white">
                <h1 className="text-3xl font-bold mb-4">Files Page</h1>
                <button
                    onClick={() => navigate("/")}
                    className="bg-gray-600 px-4 py-2 rounded hover:bg-gray-700"
                >
                    Back to Home
                </button>
            </div>
            <div className="flex flex-row flex-1 overflow-hidden">
                {/* File Table Section */}
                <div className="w-2/3 p-4 overflow-y-auto border-r border-slate-600">
                    {sortedFiles.length === 0 ? (
                        <div className="flex flex-col space-y-4">
                            <div className="shimmer" />
                            <div className="shimmer" />
                            <div className="shimmer" />
                        </div>
                    ) : (
                        <FileTable
                            files={sortedFiles}
                            selected={selected}
                            onSelect={setSelected}
                            sortField={sortField}
                            sortAsc={sortAsc}
                            onSortChange={(field) => {
                                if (field === sortField) {
                                    setSortAsc(!sortAsc);
                                } else {
                                    setSortField(field);
                                    setSortAsc(true);
                                }
                            }}
                        />
                    )}
                </div>

                {/* File Details Sidebar */}
                <AnimatePresence>
                    {selected && (
                        <SidebarMotion
                            key="sidebar"
                            initial={{ opacity: 0, x: 50 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: 50 }}
                            transition={{ type: "spring", stiffness: 300, damping: 25 }}
                            className="w-1/3 p-4 bg-white bg-opacity-10 backdrop-blur-md rounded-xl shadow-inner border border-slate-500 flex-shrink-0 h-full"
                        >
                            <FileDetailSidebar selected={selected} />
                        </SidebarMotion>
                    )}
                </AnimatePresence>
            </div>
            <div className="p-4">
                <BrowseFileButton />
            </div>
        </div>
    );
}
