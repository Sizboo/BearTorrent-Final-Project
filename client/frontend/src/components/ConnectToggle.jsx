import { useState, useEffect } from 'react';
import { Button, Stack } from '@mui/material';
import { invoke } from '@tauri-apps/api/core';

export default function ConnectToggle() {
    const [isConnected, setIsConnected] = useState(false); // default to false initially
    const [isSeeding, setIsSeeding] = useState(false);

    // Sync with backend when component mounts
    useEffect(() => {
        const interval = setInterval(() => {
            invoke('is_connected')
                .then((result) => setIsConnected(!!result))
                .catch(console.error);
        }, 1000); // check every second

        return () => clearInterval(interval);
    }, []);

    const handleConnectToggle = async () => {
        try {
            if (!isConnected) {
                await invoke('reconnect');
                await invoke('get_available_files')
                    .then((result) => {
                        // Emit a custom event to notify your App.jsx to update file list
                        window.dispatchEvent(new CustomEvent('file-refresh', { detail: result }));
                    })
                    .catch(console.error);
                setIsConnected(true);
            } else {
                await invoke('disconnect');
                setIsConnected(false);
                setIsSeeding(false);
            }
        } catch (err) {
            console.error('Connection toggle failed:', err);
        }
    };


    return (
        <Stack direction="row" spacing={2}>
            <Button
                variant={isConnected ? 'contained' : 'outlined'}
                color={isConnected ? 'success' : 'error'} // should be red when disconnected
                onClick={handleConnectToggle}
            >
                {isConnected ? 'Disconnect' : 'Connect'}
            </Button>
        </Stack>
    );
}
