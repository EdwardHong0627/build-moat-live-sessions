"""HTTP Router (FastAPI) — Markdown KB solution.
Endpoints: GET /health, POST /index, POST /chat, POST /chat/stream."""
import json
import logging
from contextlib import asynccontextmanager

from fastapi import FastAPI, HTTPException
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse, StreamingResponse

from . import answerer
from .config import get_settings
from .engine import Engine
from .llm_client import LLMClient, LLMError
from .schemas import ChatRequest, ChatResponse, IndexResponse, WikiResponse

logging.basicConfig(level=logging.INFO)
log = logging.getLogger("kb")

settings = get_settings()
engine = Engine(settings)


@asynccontextmanager
async def lifespan(_: FastAPI):
    # Build the LLM client (key required for answering). /health stays green even if
    # this fails, so liveness never depends on OpenAI.
    try:
        engine.llm = LLMClient(settings)
    except Exception as exc:  # noqa: BLE001
        log.warning("LLM client unavailable at startup: %s", exc)
    engine.load_on_startup()
    yield


app = FastAPI(title="KB Q&A Bot — Markdown KB (Strategy A)", lifespan=lifespan)


@app.exception_handler(RequestValidationError)
async def _validation_handler(_, __):
    # Honour the 'reject malformed body with 400' NFR (FastAPI defaults to 422).
    return JSONResponse(status_code=400, content={"detail": "Invalid or missing 'query'."})


@app.get("/health")
def health():
    return {"status": "ok"}


@app.post("/index", response_model=IndexResponse)
def index():
    files_indexed, sections_indexed = engine.build_index()
    return IndexResponse(files_indexed=files_indexed, sections_indexed=sections_indexed)


@app.post("/wiki", response_model=WikiResponse)
def wiki():
    """Generate wiki/index.md from the current index so topics can be browsed (FR-021)."""
    result = engine.generate_wiki()
    if result is None:
        raise HTTPException(status_code=409, detail="Knowledge base not indexed; POST /index first.")
    return WikiResponse(**result)


@app.post("/chat", response_model=ChatResponse)
def chat(req: ChatRequest):
    try:
        result = engine.chat(req.query)
    except LLMError:
        raise HTTPException(status_code=503, detail="Answer service is unavailable; please retry.")
    return ChatResponse(**result)


@app.post("/chat/stream")
def chat_stream(req: ChatRequest):
    """SSE: emit selected sources first, then answer tokens, then a 'done' event.
    Same grounding/citation rules as /chat (FR-017)."""
    prepared = engine.prepare(req.query)

    def sse(event: str, data) -> str:
        return f"event: {event}\ndata: {json.dumps(data)}\n\n"

    def generate():
        yield sse("sources", prepared.sources)
        if not prepared.indexed:
            yield sse("token", {"text": answerer.NOT_INDEXED})
            yield sse("done", {"grounded": False})
            return
        if not prepared.has_context:
            yield sse("token", {"text": answerer.CANNOT_CONFIRM})
            yield sse("done", {"grounded": False})
            return
        if engine.llm is None:
            yield sse("error", {"detail": "Answer service unavailable"})
            return
        try:
            for token in engine.llm.stream(
                answerer.SYSTEM_PROMPT, answerer.build_user_prompt(req.query, prepared.context)
            ):
                yield sse("token", {"text": token})
        except LLMError:
            yield sse("error", {"detail": "Answer service unavailable"})
            return
        yield sse("done", {"grounded": True})

    return StreamingResponse(generate(), media_type="text/event-stream")
