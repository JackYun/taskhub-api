-- Users -----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    email         TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT    NOT NULL,
    created_at    TEXT    NOT NULL
);

-- Tasks -----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title       TEXT    NOT NULL,
    description TEXT,
    status      TEXT    NOT NULL DEFAULT 'todo'
                CHECK (status IN ('todo', 'in_progress', 'done')),
    due_date    TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_user_created ON tasks (user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_user_status  ON tasks (user_id, status);
CREATE INDEX IF NOT EXISTS idx_tasks_user_due     ON tasks (user_id, due_date);

-- Tags (normalised many-to-one: a task owns a set of tag strings) ---------
CREATE TABLE IF NOT EXISTS task_tags (
    task_id INTEGER NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    tag     TEXT    NOT NULL,
    PRIMARY KEY (task_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_task_tags_tag ON task_tags (tag);
