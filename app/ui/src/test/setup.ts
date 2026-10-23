import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

afterEach(cleanup);

// jsdom doesn't lay out, so it has no scrollIntoView.
Element.prototype.scrollIntoView ??= () => {};
