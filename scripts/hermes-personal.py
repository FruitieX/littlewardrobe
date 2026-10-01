"""Resolve only the locally owned personal Hermes credential, without fallback."""
import contextlib
import importlib.util
import io
from pathlib import Path
from urllib.parse import urlsplit

def personal_credential(refresh=False):
    from hermes_constants import get_hermes_home, get_default_hermes_root
    from hermes_cli.auth import _load_auth_store
    from hermes_cli.auth_codex import _codex_access_token_is_expiring
    home = get_hermes_home().resolve()
    if home == get_default_hermes_root().resolve() or not home.is_dir():
        raise RuntimeError('A dedicated personal profile is required')
    entries = _load_auth_store().get('credential_pool', {}).get('openai-codex', [])
    if len(entries) != 1 or entries[0].get('label') != 'personal' or entries[0].get('source') not in ('manual:device_code', 'manual:loopback_pkce'):
        raise RuntimeError('Profile must own exactly one Codex login labeled personal')
    token = entries[0].get('access_token')
    base = entries[0].get('base_url') or 'https://chatgpt.com/backend-api/codex'
    route = urlsplit(base)
    if route.scheme != 'https' or route.hostname != 'chatgpt.com' or route.port not in (None, 443) or route.username or route.password or route.query or route.fragment or route.path.rstrip('/') != '/backend-api/codex':
        raise RuntimeError('Personal login must use the native Codex route')
    if not isinstance(token, str) or not token:
        raise RuntimeError('Personal login missing')
    if _codex_access_token_is_expiring(token, 90):
        if not refresh:
            raise RuntimeError('Refresh the personal Hermes login')
        from argparse import Namespace
        from hermes_cli.auth_commands import auth_refresh_command
        # Validate local ownership before entering Hermes's refresh machinery.
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            auth_refresh_command(Namespace(provider='openai-codex', target='personal'))
        return personal_credential(False)
    return token, base

def image_provider(refresh=False, model=None, background=None, size=None):
    import hermes_cli
    root = Path(hermes_cli.__file__).resolve().parent.parent
    spec = importlib.util.spec_from_file_location('littlewardrobe_personal_images', root / 'plugins/image_gen/openai-codex/__init__.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module._read_codex_credential = lambda: personal_credential(refresh)
    if model is not None:
        if model not in module.GPT_IMAGE_2_TIERS:
            raise ValueError('Unsupported image model')
        module._resolve_model = lambda: (model, module.GPT_IMAGE_2_TIERS[model])
    if background not in (None, 'opaque', 'transparent', 'auto'):
        raise ValueError('Invalid image background')
    if size not in (None, 'auto', '816x816', '1024x1024', '1536x1024', '1024x1536'):
        raise ValueError('Invalid image size')
    build = module._build_image_request
    def image_request(**kwargs):
        path, body = build(**kwargs)
        if background is not None:
            body['background'] = background
            body['output_format'] = 'png'
        if size not in (None, 'auto'):
            body['size'] = size
        return path, body
    module._build_image_request = image_request
    return module.OpenAICodexImageGenProvider()
