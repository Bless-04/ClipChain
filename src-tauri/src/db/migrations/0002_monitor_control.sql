-- Remember pauses across processes, including pause/resume between poll ticks.
CREATE TABLE monitor_control (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    pause_generation INTEGER NOT NULL DEFAULT 0
);
INSERT INTO monitor_control (id) VALUES (1);
