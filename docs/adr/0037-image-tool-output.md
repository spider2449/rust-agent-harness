# ADR 0037: Image Tool Output

Status: Accepted

Date: 2026-10-13

## Context

`ToolContent` had two variants, `Text` and `Json`. MCP servers return image
content (for example Blender's window screenshot), and `tools-mcp` passed it
on as JSON, so the model saw base64 text. That is useless to a model, large
enough to exceed message limits, and makes the context fill up quickly.
Vision models (Claude, OpenAI models, llama.cpp with a projector) accept
images in tool results, but the neutral protocol had no way to carry one.

Adding a variant changes `rah-protocol`, the process plugin protocol and
every provider plugin, so ADR 0036 requires a decision record.

## Decision

1. **Protocol.** `ToolContent` gains `Image(ImageContent)`, where
   `ImageContent { media_type, data }` carries standard padded base64 without
   a `data:` prefix. It serializes as
   `{"type": "image", "value": {"media_type": ..., "data": ...}}`.
   `IMAGE_MEDIA_TYPES` lists the accepted types: PNG, JPEG, GIF and WebP.
   SVG and other types that can carry script are excluded.
2. **Checking.** `rah_api::tools::image(media_type, data)` is the one
   constructor for untrusted images. It checks the type, the base64 alphabet
   and padding, and a limit of `MAX_IMAGE_DATA` (4 MiB of base64). It does
   not decode the bytes. `tools-mcp` turns an unacceptable MCP image into a
   short text note. The process host treats one as a malformed result. The
   process protocol stays at version 2: images are an optional item type
   that a plugin may return, `{"type": "image", "media_type", "data"}`.
3. **Providers.** Each provider maps images to its own wire format:
   `model-anthropic` uses `image` blocks inside `tool_result`, and the
   Responses plugins use `input_image` parts in `function_call_output`.
   `model-llamacpp` sends images only to a model whose `/v1/models` entry
   lists `image` input. Otherwise, and in `model-mock`, an image becomes the
   text from `rah_api::tools::describe_image`. Request size limits are
   raised to 32 MiB to make room for images. Response limits are unchanged.
4. **Context.** `context-default` sends images only within the current turn.
   Images in earlier turns are replaced by their description, and an image
   counts as a fixed `IMAGE_CHARS` in the history window.
5. **Frontends.** The session log keeps images as they were returned. The
   Web UI shows accepted raster types as `data:` images, which its CSP
   already allows (`img-src 'self' data:`). The terminal and the Markdown
   export show the description.

## Consequences

- Sessions with images cannot be read by builds older than this change.
- Session logs grow by the size of each image (at most 4 MiB of base64 per
  image).
- A text-only llama.cpp model sees a description instead of failing.
  OpenAI and Anthropic models are assumed to accept images.
- Image tools in MCP examples, such as Blender's window screenshot, can be
  exposed.
