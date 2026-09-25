-- Per-user task scoping, applied retrospectively: every task gets an
-- owner. Existing rows predate the `users` table entirely -- there is no
-- honest owner to backfill them to -- so they are cleared rather than
-- reassigned. (A real deployment would treat that as a data-migration
-- decision, not a default; here the rows are dev scaffolding.)

DELETE FROM tasks;

ALTER TABLE tasks
    ADD COLUMN user_id INT NOT NULL REFERENCES users(id);

-- Every /tasks read is now "this user's tasks", so lookups are always by
-- (user_id, id) -- the same shape the queries in `src/db/queries.rs` use.
CREATE INDEX idx_tasks_user_id ON tasks(user_id);
