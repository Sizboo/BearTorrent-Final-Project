import { Card, CardContent, Typography, Button, Stack } from "@mui/material";
import { motion } from "framer-motion";
import { FiDownload, FiTrash2 } from "react-icons/fi";
import { invoke } from "@tauri-apps/api/core";

export default function FileDetailSidebar({ selected }) {
    const handleDownload = () => {
        if (!selected?.hash) return;
        invoke("download", { hash: selected.hash })
            .then(() => alert(`Download started for "${selected.name}"`))
            .catch(console.error);
    };

    const handleDelete = () => {
        if (!selected?.hash) return;
        if (!confirm(`Are you sure you want to delete "${selected.name}"?`)) return;

        invoke("delete_file", { hash: selected.hash })
            .then(() => alert(`Deleted "${selected.name}"`))
            .catch(console.error);
    };

    return (
        <aside style={{ position: "fixed", bottom: "24px", right: "24px", width: "360px" }}>
            <motion.div
                key={selected?.name}
                initial={{ opacity: 0, y: 30, scale: 0.95 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: 20, scale: 0.9 }}
                transition={{ duration: 0.5 }}
            >
                <Card
                    sx={{
                        backgroundColor: "primary.light",
                        backdropFilter: "blur(8px)",
                        borderRadius: 3,
                        boxShadow: 5,
                        padding: 2,
                    }}
                >
                    <CardContent>
                        {selected ? (
                            <>
                                <Typography variant="h6" gutterBottom>{selected.name}</Typography>
                                <Typography variant="body2">{selected.type} • {selected.size?.toFixed(1)} MB</Typography>
                                <Typography variant="caption" display="block" sx={{ mt: 2 }}>
                                    Modified: {selected.lastModified}
                                </Typography>

                                <Stack direction="column" spacing={2} sx={{ mt: 3 }}>
                                    <Button
                                        variant="contained"
                                        color="primary"
                                        startIcon={<FiDownload />}
                                        onClick={handleDownload}
                                        fullWidth
                                    >
                                        Download
                                    </Button>
                                    <Button
                                        variant="outlined"
                                        color="error"
                                        startIcon={<FiTrash2 />}
                                        onClick={handleDelete}
                                        fullWidth
                                    >
                                        Delete
                                    </Button>
                                </Stack>
                            </>
                        ) : (
                            <Typography variant="body2" color="text.secondary">
                                Select a file to view details
                            </Typography>
                        )}
                    </CardContent>
                </Card>
            </motion.div>
        </aside>
    );
}
