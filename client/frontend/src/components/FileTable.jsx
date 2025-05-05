// src/components/FileTable.jsx

import { Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Paper } from "@mui/material";
import FileRow from "./FileRow"; // Your file row component

export default function FileTable({ files, selected, onSelect, sortField, sortAsc, onSortChange }) {
    return (
        <TableContainer component={Paper} sx={{ backgroundColor: "background.paper", borderRadius: 2, boxShadow: 3 }}>
            <Table>
                <TableHead>
                    <TableRow>
                        <TableCell></TableCell>
                        <TableCell onClick={() => onSortChange("name")} sx={{ cursor: "pointer" }}>
                            Name
                        </TableCell>
                        <TableCell onClick={() => onSortChange("size")} sx={{ cursor: "pointer" }}>
                            Size
                        </TableCell>
                        <TableCell onClick={() => onSortChange("lastModified")} sx={{ cursor: "pointer" }}>
                            Modified
                        </TableCell>
                        <TableCell>Type</TableCell>
                    </TableRow>
                </TableHead>
                <TableBody>
                    {files.length > 0 ? (
                        files.map((file, index) => (
                            <FileRow
                                key={file.name + index}
                                file={file}
                                isSelected={selected?.name === file.name}
                                onClick={() => onSelect(file)}
                            />
                        ))
                    ) : (
                        <TableRow>
                            <TableCell colSpan={5} align="center">
                                No files found
                            </TableCell>
                        </TableRow>
                    )}
                </TableBody>
            </Table>
        </TableContainer>
    );
}
