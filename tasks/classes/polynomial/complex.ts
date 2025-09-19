export class Complex {
    constructor(real: number);
    constructor(real: number, imag: number);
    constructor(public real: number, public imag: number = 0) { }
}
