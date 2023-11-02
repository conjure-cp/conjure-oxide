let currentSortHeader = null;

function IntValueComparator(header) {
    const index = header.cellIndex;
    const mult = (header.dataset.order === "desc") ? -1 : 1;

    return (a, b) => {
        const aInt = parseInt(a.cells[index].textContent);
        const bInt = parseInt(b.cells[index].textContent);
        let ans = 0;

        if (aInt > bInt) ans = 1;
        if (aInt < bInt) ans = -1;

        return ans * mult;
    }
}

function FileLengthComparator(header) {
    const index = header.cellIndex;
    const mult = (header.dataset.order === "desc") ? -1 : 1;

    return (a, b) => {
        const aSize = parseInt(a.cells[index].getAttribute("n_lines"));
        const bSize = parseInt(b.cells[index].getAttribute("n_lines"));
        let ans = 0;

        if (aSize > bSize) ans = 1;
        if (aSize < bSize) ans = -1;

        return ans * mult;
    }
}

function toggleOrder(header) {
    if (currentSortHeader !== null) {
        if (currentSortHeader !== header)
            currentSortHeader.className = "sort-none"
    }
    currentSortHeader = header;

    if (currentSortHeader.dataset.order === "desc") {
        currentSortHeader.dataset.order = "asc";
        currentSortHeader.className = "sort-asc";
    } else {
        currentSortHeader.dataset.order = "desc";
        currentSortHeader.className = "sort-desc";
    }
}

function sortRows(table, header, comparator=IntValueComparator) {
    const rows = Array.from(table.querySelectorAll("tbody tr"));
    rows.sort(comparator(header));
    rows.forEach(row => table.querySelector("tbody").appendChild(row));
}

document.addEventListener("DOMContentLoaded", function () {
    console.log("DOM Loaded!");

    const table = document.getElementById("sortable-table");
    const headers = table.querySelectorAll("th");

    headers.forEach(header => {
        header.addEventListener("click", (e) => {
            toggleOrder(header);
            if (header.id === "first-table-cell") {
                sortRows(table, header, FileLengthComparator);
            }
            else {
                sortRows(table, header);
            }
        });
    });
});
