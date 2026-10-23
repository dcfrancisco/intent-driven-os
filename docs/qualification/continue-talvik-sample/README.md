# Continue/Talvik sample project

Open this directory as a small VS Code workspace while qualifying the local
provider. The expected model is `deepseek-coder-1.3b`.

Use these prompts in order:

1. `Implement the missing function in hello.py. Reply with only a unified diff.`
2. `Review hello.py for edge cases. List two concrete issues and propose a test for each.`
3. `Add a test for hello_name in test_hello.py, then show the complete test file.`

Expected results:

- Continue uses the Talvik model entry and streams a non-empty response.
- The response refers to the files in this directory rather than inventing a
  different project.
- Talvik logs a request ID and model ID for every request, without logging the
  prompt or response.
- After cancelling a request, the next prompt completes normally.
- No other local or remote provider is configured in the qualification file.
