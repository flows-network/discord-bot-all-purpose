# Bring Your Own LLM 

This branch demonstrates how to use your own LLM in the project.

# Prerequisites

There are additionally 3 environment variables to set on the flows Setting tab.

'LLM_MODEL' - the name of your model, i.e. "llama-chat-7b"

'LLM_API_KEY' - an OPENAI_API_KEY like token, you may skip it if you run your own LLM locally and don't need to authenticate

'LLM_API_BASE' - the url at which you're providing your own LLM text generation service, it looks like "http://127.0.0.1:8080/v1" if you run locally, please don't add chat/completions to the end of the url;
