-- =============================================================================
-- V0058: McKee Main Plot / Subplot structure on plot lines.
--
-- * value_axis is the Value Axis the plot line turns. The one Main Plot's axis
--   is the accepted series Story Spine's Spine Axis; the planning validator
--   compares them (trim, then exact match).
-- * subplot_job is the closed structural job a Subplot performs toward the
--   Controlling Idea. The Main Plot carries none.
-- * main_plot marks the project's one Main Plot. Existing rows and non-McKee
--   projects keep NULL / 0, so their reads and exports are byte-identical.
--
-- The CHECK is the database backstop for the closed job vocabulary; the
-- service returns a readable error first.
-- =============================================================================

ALTER TABLE plot_line
    ADD COLUMN value_axis TEXT;

ALTER TABLE plot_line
    ADD COLUMN subplot_job TEXT
    CHECK (subplot_job IS NULL
        OR subplot_job IN ('contradict', 'resonate', 'set_up', 'complicate'));

ALTER TABLE plot_line
    ADD COLUMN main_plot INTEGER NOT NULL DEFAULT 0
    CHECK (main_plot IN (0, 1));
