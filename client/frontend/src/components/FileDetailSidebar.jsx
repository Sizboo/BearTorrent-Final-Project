import { Card, CardContent, Typography, Button } from "@mui/material";
import { motion } from "framer-motion";
import { FiDownload } from "react-icons/fi";

export default function FileDetailSidebar({ selected }) {
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
                                <Typography variant="body2">{selected.type} • {selected.size.toFixed(1)} MB</Typography>
                                <Typography variant="caption" display="block" sx={{ mt: 2 }}>
                                    Modified: {selected.lastModified}
                                </Typography>
                                <Button
                                    variant="contained"
                                    color="primary"
                                    startIcon={<FiDownload />}
                                    sx={{ mt: 3 }}
                                    fullWidth
                                >
                                    Download
                                </Button>
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
