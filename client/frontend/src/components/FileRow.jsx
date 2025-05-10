// src/components/FileRow.jsx

import { TableRow, TableCell } from "@mui/material";
import { motion } from "framer-motion";
import { FiFileText, FiImage, FiUpload, FiFolder } from "react-icons/fi";

// Create a Motion-wrapped TableRow
const MotionTableRow = motion(TableRow);

const getFileIcon = (type) => {
    switch (type) {
        case "PDF":
        case "Text": return <FiFileText color="#42A5F5" />;
        case "Image": return <FiImage color="#EC407A" />;
        case "Presentation": return <FiUpload color="#FFB300" />;
        default: return <FiFolder color="#90A4AE" />;
    }
};

export default function FileRow({ file, isSelected, onClick }) {
    return (
        <MotionTableRow
            layout
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            transition={{ duration: 0.3 }}
            hover
            selected={isSelected}
            onClick={onClick}
            sx={{
                cursor: "pointer",
                backgroundColor: isSelected ? "primary.lighter" : "inherit",
            }}
        >
            <TableCell>{getFileIcon(file.type)}</TableCell>
            <TableCell>{file.name}</TableCell>
            <TableCell>{`${file.size.toFixed(1)} MB`}</TableCell>
            <TableCell>{file.lastModified}</TableCell>
            <TableCell>{file.type}</TableCell>
        </MotionTableRow>
    );
}
