import { useEffect, useState } from 'react';
import { Button } from '@mui/material';
import { invoke } from '@tauri-apps/api/core';

export default function ToggleButton() {
    const [isToggled, setIsToggled] = useState(false);

    // Correct use of useEffect
    useEffect(() => {
        invoke("is_seeding")
            .then((enabled) => setIsToggled(!!enabled))
            .catch((err) => console.error("Failed to fetch seeding state:", err));
    }, []);

    const handleToggle = async () => {
        const newState = !isToggled;
        setIsToggled(newState);

        try {
            if (newState) {
                await invoke("start_seeding");
                console.log("Seeding started.");
            } else {
                await invoke("stop_seeding");
                console.log("Seeding stopped.");
            }
        } catch (err) {
            console.error("Seeding toggle failed:", err);
        }
    };

    return (
        <Button
            variant={isToggled ? "contained" : "outlined"}
            color={isToggled ? "success" : "inherit"}
            onClick={handleToggle}
            sx={{ ml: 2 }}
        >
            {isToggled ? "Seeding On" : "Seeding Off"}
        </Button>
    );
}
