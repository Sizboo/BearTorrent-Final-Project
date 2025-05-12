import { useState } from 'react';
import { Button, Stack } from '@mui/material';
import { invoke } from '@tauri-apps/api/core';

export default function ConnectToggle() {
    // 🟢 Define state at the top
    const [isConnected, setIsConnected] = useState(true);
    const [isSeeding, setIsSeeding] = useState(false);

    const handleConnectToggle = async () => {
        try {
            if (!isConnected) {
                await invoke('connect');
                console.log('Connected to server.');
                setIsConnected(true);
            } else {
                await invoke('disconnect');
                console.log('Disconnected from server.');
                setIsConnected(false);
                setIsSeeding(false); // stop seeding if we disconnect
            }
        } catch (err) {
            console.error('Connection toggle failed:', err);
        }
    };

    return (
        <Stack direction="row" spacing={2}>
            <Button
                variant={isConnected ? 'contained' : 'outlined'}
                color={isConnected ? 'primary' : 'inherit'}
                onClick={handleConnectToggle}
            >
                {isConnected ? 'Disconnect' : 'Connect'}
            </Button>
        </Stack>
    );
}
