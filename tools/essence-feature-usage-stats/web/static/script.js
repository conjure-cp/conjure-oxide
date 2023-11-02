let currentSortHeader = null;
let keywordRules = {};

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

function toggleCollapsibleList() {
    let listItems = document.querySelectorAll('#essence-keywords li');
    let showMoreButton = document.getElementById('show-more-button');
    let collapsibleList = document.getElementById('collapsible-list');

    if (showMoreButton.textContent === 'Show All') {
        for (let i = 0; i < listItems.length; i++) {
            listItems[i].style.display = 'list-item';
        }
        showMoreButton.textContent = 'Show Less';
    } else {
        for (let i = 5; i < listItems.length; i++) {
            listItems[i].style.display = 'none';
        }
        showMoreButton.textContent = 'Show All';
    }
}

function make_sortable_headers(table) {
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
}

function findColumnIndex(columnHeaders, columnName) {
    let columnIndex = -1;
    for (let i = 0; i < columnHeaders.length; i++) {
        const header = columnHeaders[i];
        if (header.getAttribute("data-column") === columnName) {
            columnIndex = i;
            break;
        }
    }
    return columnIndex;
}

function make_hideable_columns(table) {
    const checkboxes = document.querySelectorAll(".column-checkbox");
    const rows = table.querySelectorAll("tbody tr");

    checkboxes.forEach((checkbox) => {
        checkbox.addEventListener("change", function(e) {
            const columnName = e.target.getAttribute("data-column");
            const columnHeaders = Array.from(table.querySelector("thead").querySelectorAll("th"));

            let columnIndex = findColumnIndex(columnHeaders, columnName);

            if (columnIndex !== -1) {
                columnHeaders[columnIndex].style.display = e.target.checked ? "table-cell" : "none";
                rows.forEach(function(row) {
                    const cells = row.querySelectorAll("td");
                    cells[columnIndex].style.display = e.target.checked ? "table-cell" : "none";
                });
            }
        });
    });
}


function make_file_controls(table) {
    const radio_controls = document.querySelectorAll(".radio-controls");
    radio_controls.forEach((group) => {
        const radio_buttons = Array.from(group.getElementsByTagName("input"));
        radio_buttons.forEach((button) => {
            button.addEventListener("change", (e) => {
                const columnName = e.target.parentElement.getAttribute("data-column");
                keywordRules[columnName] = e.target.value;
                updateRowVisibility(table);
            })
        })
    })
}


function updateRowVisibility(table) {
    const columnHeaders = Array.from(table.querySelector("thead").querySelectorAll("th"));
    const rows = table.querySelectorAll("tbody tr");

     rows.forEach((row) => {
         row.hidden = false;
     });

    for (let columnName of Object.keys(keywordRules)) {
        const option = keywordRules[columnName];
        const columnIndex = findColumnIndex(columnHeaders, columnName);

        rows.forEach((row) => {
            const cells = row.querySelectorAll("td");
            const usages = parseInt(cells[columnIndex].textContent);

            if (option === "exclude") {
                row.hidden = (usages > 0) || row.hidden;
            } else if (option === "require") {
                row.hidden = (usages === 0) || row.hidden;
            }
        });
    }
}


document.addEventListener("DOMContentLoaded", function () {
    console.log("DOM Loaded!");

    const table = document.getElementById("sortable-table");
    make_sortable_headers(table);
    make_hideable_columns(table);
    make_file_controls(table);
});
