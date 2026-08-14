-- Mark the post-wizard spotlight tour complete for vaults that already finished setup
-- before v1.1 shipped the tour (avoids ambushing existing users on upgrade).
INSERT INTO settings (key, value)
SELECT 'first_start_tour_v1_completed', 'true'
WHERE EXISTS (SELECT 1 FROM settings WHERE key = 'wizard_v1_completed' AND value = 'true')
  AND NOT EXISTS (SELECT 1 FROM settings WHERE key = 'first_start_tour_v1_completed');
