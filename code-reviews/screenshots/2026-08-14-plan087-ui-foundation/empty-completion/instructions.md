# Clay UI review capture

- Fixture: ui-review-completion
- State: empty completion result after `zzzz` prefix
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

The live client received `CompletionResult { status: Empty, items: [] }` and
removed the completion overlay without showing a blocking empty-results panel.
