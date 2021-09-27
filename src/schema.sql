CREATE TABLE IF NOT EXISTS investments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scraped_at DATETIME NOT NULL DEFAULT (strftime('%s', 'now')),
    name TEXT NOT NULL,
    ongoing_charge DECIMAL NOT NULL,
    units DECIMAL NOT NULL,
    avg_unit_cost DECIMAL NOT NULL,
    last_price DECIMAL NOT NULL,
    total_cost DECIMAL NOT NULL,
    value DECIMAL NOT NULL,
    change DECIMAL NOT NULL
);
