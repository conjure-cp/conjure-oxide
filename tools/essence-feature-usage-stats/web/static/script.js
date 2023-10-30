function IntValueComparator(header) {
    const index = header.cellIndex;
    const mult = (header.dataset.order === "desc") ? -1 : 1;

    return (a, b) => {
        const aInt = parseInt(a.cells[index].textContent);
        const bInt = parseInt(b.cells[index].textContent);
        let ans = 0;

        if (aInt > bInt) ans = 1;
        if (aInt < bInt) ans = -1;
        ans *= mult;

        return ans;
    }
}

function toggleOrder(header) {
    if (header.dataset.order === "desc") {
        header.dataset.order = "asc";
        header.classList.remove("sort-desc");
        header.classList.add("sort-asc");
    } else {
        header.dataset.order = "desc";
        header.classList.remove("sort-asc");
        header.classList.add("sort-desc");
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
            sortRows(table, header);
        });
    });
});