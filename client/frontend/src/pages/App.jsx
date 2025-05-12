import { useEffect, useState } from "react";
import React from 'react';
import { useNavigate } from 'react-router-dom';
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "framer-motion";
import { invoke } from '@tauri-apps/api/core';
import FileTable from "../components/FileTable";
import FileDetailSidebar from "../components/FileDetailSidebar";
import ToggleButton from '../components/ToggleButton';
import ConnectToggle from '../components/ConnectToggle';
import "../index.css";
import "../shimmer.css";
import '../modern-styles.css';

console.log("Is Tauri environment:", "__TAURI_IPC__" in window);

const sidebarMotion = motion.create("div");

const initialFiles = [
    { name: "report.pdf", size: 1.2, type: "PDF", lastModified: "2023-09-12" },
    { name: "presentation.pptx", size: 4.5, type: "Presentation", lastModified: "2023-09-10" },
    { name: "photo.jpg", size: 2.1, type: "Image", lastModified: "2023-09-14" },
    { name: "notes.txt", size: 0.5, type: "Text", lastModified: "2023-09-05" },
];



export default function App() {
    const navigate = useNavigate();
    const [message, setMessage] = useState("Click to say hello...");
    const [files, setFiles] = useState([]);


    //File Updating
    useEffect(() => {
        const unlisten = listen("file-update", (event) => {
            console.log("Received file-update event C0001", event.payload);
            setFiles(event.payload); // this should be the Vec<SerializableFileInfo>
        });

        return () => {
            unlisten.then((off) => off()); // clean up listener on unmount
        };
    }, []);

    function refreshFiles() {
        setLoading(true);
        invoke("get_available_files")
            .then((result) => {
                console.log("Refreshed files:", result);
                setFiles(result);
            })
            .catch((err) => {
                console.error("Failed to refresh files:", err);
            })
            .finally(() => {
                setLoading(false);
            });
    }



    function handleHello() {
        invoke("say_hello")
            .then(alert)
            .catch(console.error);
    }

    function handleUploadClick() {
        handleClick();
        handleGreetings();
    }

    function handleGreetings() {
        invoke("say_hello")
            .then(alert)
            .catch(console.error);
    }

    const handleClick = async () => {
        console.log("[JS] Calling Rust async command...");
        setMessage("Waiting for Rust...");
        const result = await invoke("say_hello_delayed");
        console.log("[JS] Got result from Rust:", result);
        setMessage(result);
    };






    const [selected, setSelected] = useState(null);
    const [sortField, setSortField] = useState("name");
    const [sortAsc, setSortAsc] = useState(true);
    const [loading, setLoading] = useState(false);


    const sortedFiles = [...files].sort((a, b) => {
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
                    <ConnectToggle/>
                </nav>
            </header>

            {/* Main Content Layout */}
            <div className="flex flex-row flex-1 overflow-hidden">
                {/* File Table Section */}
                <div className="w-2/3 p-4 overflow-y-auto border-r border-slate-600">
                    <div className="flex justify-end mb-4">
                        <button
                            className="menu-button rounded-lg px-4 py-2 bg-blue-500 text-white hover:bg-blue-600 transition-all duration-150 flex items-center gap-2"
                            onClick={refreshFiles}
                            disabled={loading}
                        >
                            {loading && (
                                <svg className="animate-spin h-5 w-5 text-white" viewBox="0 0 24 24">
                                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
                                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v4l4-4-4-4v4a10 10 0 00-10 10h4z" />
                                </svg>
                            )}
                            {loading ? "Refreshing..." : "Refresh"}
                        </button>

                    </div>

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
                        <motion.div
                            key="sidebar"
                            initial={{ opacity: 0, x: 50 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: 50 }}
                            transition={{ type: "spring", stiffness: 300, damping: 25 }}
                            className="w-1/3 p-4 bg-white bg-opacity-10 backdrop-blur-md rounded-xl shadow-inner border border-slate-500 flex-shrink-0 h-full"
                        >
                            <FileDetailSidebar
                                selected={selected}
                                onDelete={(deletedHash) => setFiles(files.filter(f => f.hash !== deletedHash))}
                            />
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>
        </div>
    );
}
