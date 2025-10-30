// Setup file for Jest tests
/* eslint-disable max-classes-per-file */

// Mock ResizeObserver
global.ResizeObserver = class ResizeObserver {
  /* eslint-disable class-methods-use-this */
  observe() {}

  unobserve() {}

  disconnect() {}
  /* eslint-enable class-methods-use-this */
};

// Mock DOMRect
global.DOMRect = class DOMRect {
  constructor(
    public x = 0,
    public y = 0,
    public width = 0,
    public height = 0,
  ) {
    this.left = x;
    this.right = x + width;
    this.top = y;
    this.bottom = y + height;
  }

  static fromRect(rect?: {x?: number; y?: number; width?: number; height?: number}): DOMRect {
    return new DOMRect(rect?.x, rect?.y, rect?.width, rect?.height);
  }

  left = 0;

  right = 0;

  top = 0;

  bottom = 0;

  toJSON() {
    return JSON.stringify(this);
  }
};

// Mock WebGL2 context
const mockWebGL2Context = {
  getParameter: jest.fn().mockReturnValue(16384), // MAX_TEXTURE_SIZE
  canvas: {},
  drawingBufferWidth: 800,
  drawingBufferHeight: 600,
} as unknown as WebGL2RenderingContext;

// Mock HTMLCanvasElement.getContext
HTMLCanvasElement.prototype.getContext = jest.fn((contextId) => {
  if (contextId === "webgl2") {
    return mockWebGL2Context;
  }
  return null;
}) as any;

// Mock Element.scrollTo
Element.prototype.scrollTo = jest.fn();

// Mock ipcRenderer
jest.mock("electron", () => ({
  ipcRenderer: {
    on: jest.fn(),
    off: jest.fn(),
    send: jest.fn(),
    invoke: jest.fn(),
    sendSync: jest.fn(),
    removeListener: jest.fn(),
    removeAllListeners: jest.fn(),
  },
}));
