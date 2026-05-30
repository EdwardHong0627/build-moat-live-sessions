"""Index persistence — writes the FAISS index plus an inspectable metadata.json mapping
each vector to its filename#heading, and reloads both on startup (FR-010/FR-012/FR-013).
metadata.json also records the embedding model so a model change can force a re-index."""
import json
import os

from .indexer import Chunk

_META = "metadata.json"
_INDEX = "index.faiss"


def save(faiss_dir: str, index, chunks: list[Chunk], embed_model: str, dim: int) -> None:
    import faiss

    os.makedirs(faiss_dir, exist_ok=True)
    faiss.write_index(index, os.path.join(faiss_dir, _INDEX))
    meta = {
        "version": 1,
        "strategy": "vector_rag",
        "embed_model": embed_model,
        "dim": dim,
        "chunks": [
            {"filename": c.filename, "heading": c.heading, "slug": c.slug,
             "text": c.text, "chunk_id": c.chunk_id}
            for c in chunks
        ],
    }
    with open(os.path.join(faiss_dir, _META), "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=2, ensure_ascii=False)


def load(faiss_dir: str):
    """Return (faiss_index, chunks, meta) or None if absent/corrupt (degrade to
    'needs indexing' rather than crashing — Availability NFR)."""
    index_path = os.path.join(faiss_dir, _INDEX)
    meta_path = os.path.join(faiss_dir, _META)
    if not (os.path.exists(index_path) and os.path.exists(meta_path)):
        return None
    try:
        import faiss

        index = faiss.read_index(index_path)
        with open(meta_path, encoding="utf-8") as f:
            meta = json.load(f)
        chunks = [
            Chunk(d["filename"], d["heading"], d["slug"], d["text"], d["chunk_id"])
            for d in meta["chunks"]
        ]
        return index, chunks, meta
    except Exception:
        return None
