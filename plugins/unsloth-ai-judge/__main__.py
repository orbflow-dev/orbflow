import sys, os

_here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _here)
sys.path.insert(0, os.path.join(_here, '..', '..', 'sdks', 'python', 'src'))

from orbflow_sdk.subprocess_runner import run_subprocess
from orbflow_sdk.decorators import get_plugin_meta
import main as plugin_mod

cls = next(
    getattr(plugin_mod, a)
    for a in dir(plugin_mod)
    if isinstance(getattr(plugin_mod, a), type) and get_plugin_meta(getattr(plugin_mod, a))
)
run_subprocess(cls)
