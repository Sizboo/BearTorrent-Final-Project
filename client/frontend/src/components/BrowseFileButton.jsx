import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';

function BrowseFileButton() {
    const [filePath, setFilePath] = useState(null);

    const handleBrowse = async () => {
        const selected = await open({
            directory: false,
            multiple: false,
        });

        if (selected) {
            console.log("Selected file:", selected);
            setFilePath(selected); // store in state
        }
    };

    return (
        <div className="space-y-2">
            <button
                onClick={handleBrowse}
                className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition"
            >
                Browse File
            </button>
            {filePath && (
                <p className="text-sm text-gray-700 break-all">Selected: {filePath}</p>
            )}
        </div>
    );
}

export default BrowseFileButton;
