// All control implementations and internal icons are bundled locally for the Tauri webview.
import '@awesome.me/webawesome/dist/styles/themes/default.css';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import '@awesome.me/webawesome/dist/components/progress-bar/progress-bar.js';
import '@awesome.me/webawesome/dist/components/spinner/spinner.js';
import '@awesome.me/webawesome/dist/components/popover/popover.js';
import '@awesome.me/webawesome/dist/components/details/details.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/translations/zh-cn.js';
import '@awesome.me/webawesome/dist/translations/ja.js';

import { registerTranslation } from '@awesome.me/webawesome/dist/utilities/localize.js';
import english from '@awesome.me/webawesome/dist/translations/en.js';
import { registerIconLibrary } from '@awesome.me/webawesome/dist/components/icon/library.js';
import systemIcons from '@awesome.me/webawesome/dist/components/icon/library.system.js';
import { createElement, Settings, RefreshCw, X, ChevronLeft, ChevronUp, ChevronDown } from 'lucide';

const nodes = { settings: Settings, refresh: RefreshCw, close: X, back: ChevronLeft, up: ChevronUp, down: ChevronDown };
const icons = Object.fromEntries(Object.entries(nodes).map(([name, node]) => [
  name, `data:image/svg+xml,${encodeURIComponent(createElement(node, { width: 24, height: 24, 'stroke-width': 1.8 }).outerHTML)}`,
]));
registerIconLibrary('lucide', { resolver: name => icons[name] ?? icons.close });
// A future <wa-icon> without a library must also stay offline.
registerIconLibrary('default', systemIcons);

// The library currently has no Korean catalog; provide the terms used by these controls.
const korean: typeof english = { ...english, $code: 'ko', $name: '한국어',
  loading: '불러오는 중', progress: '진행률', close: '닫기', clearEntry: '입력 지우기',
};
registerTranslation(korean);
