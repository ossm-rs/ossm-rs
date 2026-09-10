# v6.5.33 - Owner UI range fix

- Backend 900 mm/s / 400 mm machine limits were already correct in v6.5.32.
- Fixed stale owner-page HTML limits that still used 600 mm/s / 180 mm.
- Fixed stale client-side validation that rejected values above 600 / 180 before the API call.
- Updated static owner-limit slider maxima to 900 / 400; runtime still replaces them with the saved machine limits.
- No motion-control changes.
