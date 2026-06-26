"""Python fallback client for the ccb-runtime-accelerator sidecar."""

from .client import AcceleratorError, call, call_or_fallback, default_socket_path

__all__ = ["AcceleratorError", "call", "call_or_fallback", "default_socket_path"]
