"""ActantDB Python client SDK.

Talks to the actantdb-server HTTP API. Mirrors the TS SDK surface.
"""

from .client import ActantClient, ActantError

__all__ = ["ActantClient", "ActantError"]
__version__ = "0.0.1"
