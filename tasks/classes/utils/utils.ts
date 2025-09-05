export function hypotenuse(a: number, b: number): number {
    let r = 0;
    if (Math.abs(a) > Math.abs(b)) {
        r = b / a;
        let c = Math.abs(a) * Math.sqrt(1 + r * r);
        return c;
    }
    if (b !== 0) {
        r = a / b;
        let c = Math.abs(b) * Math.sqrt(1 + r * r);
        return c;
    }
    return 0;
}
