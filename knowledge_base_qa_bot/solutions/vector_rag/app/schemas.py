"""Request/response contract — identical to the Markdown KB solution (strategy-agnostic)."""
from pydantic import BaseModel, Field


class ChatRequest(BaseModel):
    query: str = Field(..., min_length=1, max_length=2000)


class Source(BaseModel):
    citation: str  # filename#heading
    score: float


class ChatResponse(BaseModel):
    answer: str
    sources: list[Source]
    grounded: bool


class IndexResponse(BaseModel):
    files_indexed: int
    sections_indexed: int


class WikiResponse(BaseModel):
    path: str
    files: int
    topics: int
