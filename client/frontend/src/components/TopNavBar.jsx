import { AppBar, Toolbar, Button, Typography } from "@mui/material";
import ToggleButton from "./ToggleButton";
import ConnectToggle from "./ConnectToggle"; // ✅ Adjust path based on your folder structure

export default function TopNavBar({ activeTab, onTabChange }) {
    const tabs = ["Files", "Uploads", "Settings"];

    return (
        <AppBar
            position="static"
            sx={{
                background: "linear-gradient(to right, #1976d2, #42a5f5)",
                boxShadow: 3,
            }}
        >
            <Toolbar sx={{ display: "flex", justifyContent: "space-between" }}>
                <Typography variant="h6" component="div" sx={{ display: "flex", alignItems: "center", gap: 1 }}>
                    📁 File Manager
                </Typography>

                <div style={{ display: "flex", alignItems: "center" }}>
                    {tabs.map((tab) => (
                        <Button
                            key={tab}
                            variant={activeTab === tab ? "contained" : "text"}
                            color={activeTab === tab ? "warning" : "inherit"}
                            onClick={() => onTabChange(tab)}
                            sx={{ mx: 1 }}
                        >
                            {tab}
                        </Button>
                    ))}

                    <ToggleButton />
                    <ConnectToggle/>
                </div>
            </Toolbar>
        </AppBar>
    );
}
