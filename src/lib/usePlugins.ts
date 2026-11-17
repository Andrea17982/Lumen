import { useEffect, useState } from 'react';
import { call, desktop } from './bridge';
import { NO_CONTRIBUTIONS, PLUGINS_CHANGED, type Contributions } from './plugins';

/** Enabled plugins' contributions; reloads whenever the Plugins page changes something. */
export function usePluginContributions(): Contributions {
  const [value, setValue] = useState(NO_CONTRIBUTIONS);
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    const load = () =>
      void call<Contributions>('plugin_contributions')
        .then((next) => alive && setValue(next))
        .catch(() => alive && setValue(NO_CONTRIBUTIONS));
    load();
    window.addEventListener(PLUGINS_CHANGED, load);
    return () => {
      alive = false;
      window.removeEventListener(PLUGINS_CHANGED, load);
    };
  }, []);
  return value;
}
