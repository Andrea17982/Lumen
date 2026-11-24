import { useEffect } from 'react';
import { useSyncState } from '../../sync/react';
import { themeCss, usePluginTheme } from './theme';

/** Applies the chosen plugin theme while its plugin is on; otherwise the style's own tokens show. */
export function usePluginThemeStyle() {
  const [chosen] = usePluginTheme();
  const themes = useSyncState((st) => st.contributions.themes);
  const theme = themes.find((x) => `${x.plugin}/${x.id}` === chosen);
  useEffect(() => {
    if (!theme) return;
    const style = document.createElement('style');
    style.dataset.pluginTheme = chosen ?? '';
    style.textContent = themeCss(theme);
    document.head.append(style);
    return () => style.remove();
  }, [theme, chosen]);
}
