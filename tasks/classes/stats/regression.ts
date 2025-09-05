/**
* Determine the coefficient of determination (r^2) of a fit from the observations
* and predictions.
*
* @param {Array<Array<number>>} data - Pairs of observed x-y values
* @param {Array<Array<number>>} results - Pairs of observed predicted x-y values
*
* @return {number} - The r^2 value, or NaN if one cannot be calculated.
*/
function determinationCoefficient(data, results) {
    const predictions = [];
    const observations = [];

    data.forEach((d, i) => {
        if (d[1] !== null) {
            observations.push(d);
            predictions.push(results[i]);
        }
    });

    const sum = observations.reduce((a, observation) => a + observation[1], 0);
    const mean = sum / observations.length;

    const ssyy = observations.reduce((a, observation) => {
        const difference = observation[1] - mean;
        return a + (difference * difference);
    }, 0);

    const sse = observations.reduce((accum, observation, index) => {
        const prediction = predictions[index];
        const residual = observation[1] - prediction[1];
        return accum + (residual * residual);
    }, 0);

    return 1 - (sse / ssyy);
}

/**
* Determine the solution of a system of linear equations A * x = b using
* Gaussian elimination.
*
* @param {Array<Array<number>>} input - A 2-d matrix of data in row-major form [ A | b ]
* @param {number} order - How many degrees to solve for
*
* @return {Array<number>} - Vector of normalized solution coefficients matrix (x)
*/
function gaussianElimination(input, order) {
    const matrix = input;
    const n = input.length - 1;
    const coefficients = [order];

    for (let i = 0; i < n; i++) {
        let maxrow = i;
        for (let j = i + 1; j < n; j++) {
            if (Math.abs(matrix[i][j]) > Math.abs(matrix[i][maxrow])) {
                maxrow = j;
            }
        }

        for (let k = i; k < n + 1; k++) {
            const tmp = matrix[k][i];
            matrix[k][i] = matrix[k][maxrow];
            matrix[k][maxrow] = tmp;
        }

        for (let j = i + 1; j < n; j++) {
            for (let k = n; k >= i; k--) {
                matrix[k][j] -= (matrix[k][i] * matrix[i][j]) / matrix[i][i];
            }
        }
    }

    for (let j = n - 1; j >= 0; j--) {
        let total = 0;
        for (let k = j + 1; k < n; k++) {
            total += matrix[k][j] * coefficients[k];
        }

        coefficients[j] = (matrix[n][j] - total) / matrix[j][j];
    }

    return coefficients;
}

/**
* Round a number to a precision, specificed in number of decimal places
*
* @param {number} number - The number to round
* @param {number} precision - The number of decimal places to round to:
*                             > 0 means decimals, < 0 means powers of 10
*
*
* @return {numbr} - The number, rounded
*/
function round(number, precision) {
    const factor = 10 ** precision;
    return Math.round(number * factor) / factor;
}

export class RegressionOptions {
    constructor(public precision: number, public order: number) {
    }
}

export class LinearRegression {
    public gradient: number;
    public intercept: number;
    public points: number[];
    public string: string;
    public r2: number;
    public options: RegressionOptions;
    public equation: number[];

    public constructor(data: number[][]) {
        this.options = new RegressionOptions(2, 2);

        const sum = [0, 0, 0, 0, 0];
        let len = 0;

        for (let n = 0; n < data.length; n++) {
            if (data[n][1] !== null) {
                len++;
                sum[0] += data[n][0];
                sum[1] += data[n][1];
                sum[2] += data[n][0] * data[n][0];
                sum[3] += data[n][0] * data[n][1];
                sum[4] += data[n][1] * data[n][1];
            }
        }

        const run = ((len * sum[2]) - (sum[0] * sum[0]));
        const rise = ((len * sum[3]) - (sum[0] * sum[1]));
        const gradient = run === 0 ? 0 : round(rise / run, this.options.precision);
        const intercept = round((sum[1] / len) - ((gradient * sum[0]) / len), this.options.precision);

        this.gradient = gradient;
        this.intercept = intercept;
        this.points = data.map(point => this.predict(point[0]));
        this.equation = [gradient, intercept];
        this.r2 = round(determinationCoefficient(data, this.points), this.options.precision);
        this.string = intercept === 0 ? `y = ${gradient}x` : `y = ${gradient}x + ${intercept}`;
    }

    public predict(x: number): number {
        return round((this.gradient * x) + this.intercept, this.options.precision)
    }
}

export class ExponentialRegression {
    public coeffA: number;
    public coeffB: number;
    public points: number[];
    public string: string;
    public r2: number;
    public options: RegressionOptions;
    public equation: number[];

    public constructor(data: number[][]) {
        this.options = new RegressionOptions(2, 2);
        const sum = [0, 0, 0, 0, 0, 0];

        for (let n = 0; n < data.length; n++) {
            if (data[n][1] !== null) {
                sum[0] += data[n][0];
                sum[1] += data[n][1];
                sum[2] += data[n][0] * data[n][0] * data[n][1];
                sum[3] += data[n][1] * Math.log(data[n][1]);
                sum[4] += data[n][0] * data[n][1] * Math.log(data[n][1]);
                sum[5] += data[n][0] * data[n][1];
            }
        }

        const denominator = ((sum[1] * sum[2]) - (sum[5] * sum[5]));
        const a = Math.exp(((sum[2] * sum[3]) - (sum[5] * sum[4])) / denominator);
        const b = ((sum[1] * sum[4]) - (sum[5] * sum[3])) / denominator;

        this.coeffA = round(a, this.options.precision);;
        this.coeffB = round(b, this.options.precision);;
        this.points = data.map(point => this.predict(point[0]));
        this.equation = [this.coeffA, this.coeffB];
        this.string = `y = ${this.coeffA}e^(${this.coeffB}x)`;
        this.r2 = round(determinationCoefficient(data, this.points), this.options.precision);
    }

    public predict(x: number): number {
        return round(this.coeffA * Math.exp(this.coeffB * x), this.options.precision)
    }
}

export class LogarithmicRegression {
    public coeffA: number;
    public coeffB: number;
    public points: number[];
    public string: string;
    public r2: number;
    public options: RegressionOptions;
    public equation: number[];

    public constructor(data: number[][]) {
        this.options = new RegressionOptions(2, 2);
        const sum = [0, 0, 0, 0];
        const len = data.length;
        for (let n = 0; n < len; n++) {
            if (data[n][1] !== null) {
                sum[0] += Math.log(data[n][0]);
                sum[1] += data[n][1] * Math.log(data[n][0]);
                sum[2] += data[n][1];
                sum[3] += (Math.log(data[n][0]) ** 2);
            }
        }

        const a = ((len * sum[1]) - (sum[2] * sum[0])) / ((len * sum[3]) - (sum[0] * sum[0]));
        this.coeffB = round(a, this.options.precision);
        this.coeffA = round((sum[2] - (this.coeffB * sum[0])) / len, this.options.precision);

        this.points = data.map(point => this.predict(point[0]));
        this.equation = [this.coeffA, this.coeffB];
        this.string = `y = ${this.coeffA} + ${this.coeffB} ln(x)`;
        this.r2 = round(determinationCoefficient(data, this.points), this.options.precision);
    }

    public predict(x: number): number {
        return round(this.coeffA + (this.coeffB * Math.log(x)), this.options.precision)
    }
}

export class PowerRegression {
    public coeffA: number;
    public coeffB: number;
    public points: number[];
    public string: string;
    public r2: number;
    public options: RegressionOptions;
    public equation: number[];

    public constructor(data: number[][]) {
        this.options = new RegressionOptions(2, 2);
        const sum = [0, 0, 0, 0, 0];
        const len = data.length;

        for (let n = 0; n < len; n++) {
            if (data[n][1] !== null) {
                sum[0] += Math.log(data[n][0]);
                sum[1] += Math.log(data[n][1]) * Math.log(data[n][0]);
                sum[2] += Math.log(data[n][1]);
                sum[3] += (Math.log(data[n][0]) ** 2);
            }
        }

        const b = ((len * sum[1]) - (sum[0] * sum[2])) / ((len * sum[3]) - (sum[0] ** 2));
        const a = ((sum[2] - (b * sum[0])) / len);
        this.coeffA = round(Math.exp(a), this.options.precision);
        this.coeffB = round(b, this.options.precision);

        this.points = data.map(point => this.predict(point[0]));
        this.equation = [this.coeffA, this.coeffB];
        this.string = `y = ${this.coeffA}x^${this.coeffB}`;
        this.r2 = round(determinationCoefficient(data, this.points), this.options.precision);
    }

    public predict(x: number): number {
        return round(this.coeffA * (x ** this.coeffB), this.options.precision)
    }
}

export class PolynomialRegression {
    public coefficients: number[];
    public points: number[];
    public string: string;
    public r2: number;
    public options: RegressionOptions;
    public equation: number[];

    public constructor(data: number[][]) {
        this.options = new RegressionOptions(2, 2);
        const lhs = [];
        const rhs = [];
        let a = 0;
        let b = 0;
        const len = data.length;
        const k = this.options.order + 1;

        for (let i = 0; i < k; i++) {
            for (let l = 0; l < len; l++) {
                if (data[l][1] !== null) {
                    a += (data[l][0] ** i) * data[l][1];
                }
            }

            lhs.push(a);
            a = 0;

            const c = [];
            for (let j = 0; j < k; j++) {
                for (let l = 0; l < len; l++) {
                    if (data[l][1] !== null) {
                        b += data[l][0] ** (i + j);
                    }
                }
                c.push(b);
                b = 0;
            }
            rhs.push(c);
        }
        rhs.push(lhs);

        const coefficients = gaussianElimination(rhs, k).map(v => round(v, this.options.precision));

        this.coefficients = coefficients;
        this.points = data.map(point => this.predict(point[0]));

        let string = 'y = ';
        for (let i = coefficients.length - 1; i >= 0; i--) {
            if (i > 1) {
                string += `${coefficients[i]}x^${i} + `;
            } else if (i === 1) {
                string += `${coefficients[i]}x + `;
            } else {
                string += coefficients[i];
            }
        }

        this.string = string;
        this.equation = [...coefficients].reverse();
        this.r2 = round(determinationCoefficient(data, this.points), this.options.precision);
    }

    public predict(x: number): number {
        let sum = 0;
        for (let i = 0; i < this.coefficients.length; i++) {
            sum += this.coefficients[i] * (x ** i);
        }
        return round(sum, this.options.precision);
    }
}
