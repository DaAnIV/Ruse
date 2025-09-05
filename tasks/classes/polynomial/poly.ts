class Polynomial {

    constructor(public readonly coeffs: Map<number, number>) { }

    private clone_coeffs() {
        var res = {};
        for (var i in this['coeffs']) {
            res[i] = this['coeffs'][i];
        }
        return res;
    }

    public degree() {
        var max = -1;
        for (var i of this['coeffs'].keys()) {
            if (this['coeffs'][i] != 0) {
                max = Math.max(max, i);
            }
        }
        return max;
    }

    public real_coeffs() {
        let deg = this.degree();
        let real_coeffs = new Array(deg + 1);
        for (let i = 0; i < deg + 1; i++) {
            real_coeffs[i] = this['coeffs'].get(i) || 0;
        }

        return real_coeffs;
    }
}

export { Polynomial };
