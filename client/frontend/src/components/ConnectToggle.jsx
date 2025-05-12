import { useState, useEffect } from 'react';
import { Button, Stack } from '@mui/material';
import { invoke } from '@tauri-apps/api/core';

export default function ConnectToggle() {
    const [isConnected, setIsConnected] = useState(false); // default to false initially
    const [isSeeding, setIsSeeding] = useState(false);

    // ✅ Sync with backend when component mounts
    useEffect(() => {
        invoke('is_connected')
            .then((result) => {
                console.log('Backend says connected?', result);
                setIsConnected(!!result);
            })
            .catch((err) => {
                console.error('Failed to check connection status:', err);
            });
    }, []);

    const handleConnectToggle = async () => {
        try {
            if (!isConnected) {
                await invoke('reconnect');
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
