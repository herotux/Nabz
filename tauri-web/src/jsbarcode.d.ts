declare module 'jsbarcode' {
  type JsBarcodeOptions = {
    format?: string;
    displayValue?: boolean;
    lineColor?: string;
    width?: number;
    height?: number;
    margin?: number;
    font?: string;
    fontSize?: number;
  };
  function JsBarcode(element: SVGElement | HTMLCanvasElement | string, value: string, options?: JsBarcodeOptions): void;
  export default JsBarcode;
}
