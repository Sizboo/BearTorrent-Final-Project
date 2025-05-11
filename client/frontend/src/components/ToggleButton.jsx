import { useState } from 'react';
import { Button } from '@mui/material';

export default function ToggleButton() {
    const [isToggled, setIsToggled] = useState(false);

    const handleToggle = () => {
        setIsToggled(prev => !prev);
        // Optional: invoke Tauri command
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
