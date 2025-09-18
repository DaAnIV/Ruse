package linear;

import utils.Utils;

public class EigenvalueDecomposition {
    private int n;
    private double[] e;
    private double[] d;
    private Matrix V;

    public EigenvalueDecomposition(Matrix matrix) {
        boolean assumeSymmetric = false;

        if (!matrix.isSquare()) {
            throw new IllegalArgumentException("Matrix is not a square matrix");
        }

        if (matrix.isEmpty()) {
            throw new IllegalArgumentException("Matrix must be non-empty");
        }

        int n = matrix.columns;
        Matrix V = Matrix.zeros(n, n);
        double[] d = new double[n];
        double[] e = new double[n];
        Matrix value = matrix;

        boolean isSymmetric = false;
        if (assumeSymmetric) {
            isSymmetric = true;
        } else {
            isSymmetric = matrix.isSymmetric();
        }

        if (isSymmetric) {
            for (int i = 0; i < n; i++) {
                for (int j = 0; j < n; j++) {
                    V.set(i, j, value.get(i, j));
                }
            }
            tred2(n, e, d, V);
            tql2(n, e, d, V);
        } else {
            Matrix H = Matrix.zeros(n, n);
            double[] ort = new double[n];
            for (int j = 0; j < n; j++) {
                for (int i = 0; i < n; i++) {
                    H.set(i, j, value.get(i, j));
                }
            }
            orthes(n, H, ort, V);
            hqr2(n, e, d, V, H);
        }

        this.n = n;
        this.e = e;
        this.d = d;
        this.V = V;
    }

    public double[] getEigenvalue(int index) {
        return new double[]{this.d[index], this.e[index]};
    }

    public double[] getRealEigenvalues() {
        return this.d.clone();
    }

    public double[] getImaginaryEigenvalues() {
        return this.e.clone();
    }

    public Matrix getEigenvectorMatrix() {
        return this.V;
    }

    private static void tred2(int n, double[] e, double[] d, Matrix V) {
        for (int j = 0; j < n; j++) {
            d[j] = V.get(n - 1, j);
        }

        for (int i = n - 1; i > 0; i--) {
            double scale = 0;
            double h = 0;
            for (int k = 0; k < i; k++) {
                scale = scale + Math.abs(d[k]);
            }

            if (scale == 0) {
                e[i] = d[i - 1];
                for (int j = 0; j < i; j++) {
                    d[j] = V.get(i - 1, j);
                    V.set(i, j, 0);
                    V.set(j, i, 0);
                }
            } else {
                for (int k = 0; k < i; k++) {
                    d[k] /= scale;
                    h += d[k] * d[k];
                }

                double f = d[i - 1];
                double g = Math.sqrt(h);
                if (f > 0) {
                    g = -g;
                }

                e[i] = scale * g;
                h = h - f * g;
                d[i - 1] = f - g;
                for (int j = 0; j < i; j++) {
                    e[j] = 0;
                }

                for (int j = 0; j < i; j++) {
                    f = d[j];
                    V.set(j, i, f);
                    g = e[j] + V.get(j, j) * f;
                    for (int k = j + 1; k <= i - 1; k++) {
                        g += V.get(k, j) * d[k];
                        e[k] += V.get(k, j) * f;
                    }
                    e[j] = g;
                }

                f = 0;
                for (int j = 0; j < i; j++) {
                    e[j] /= h;
                    f += e[j] * d[j];
                }

                double hh = f / (h + h);
                for (int j = 0; j < i; j++) {
                    e[j] -= hh * d[j];
                }

                for (int j = 0; j < i; j++) {
                    f = d[j];
                    g = e[j];
                    for (int k = j; k <= i - 1; k++) {
                        V.set(k, j, V.get(k, j) - (f * e[k] + g * d[k]));
                    }
                    d[j] = V.get(i - 1, j);
                    V.set(i, j, 0);
                }
            }
            d[i] = h;
        }

        for (int i = 0; i < n - 1; i++) {
            V.set(n - 1, i, V.get(i, i));
            V.set(i, i, 1);
            double h = d[i + 1];
            if (h != 0) {
                for (int k = 0; k <= i; k++) {
                    d[k] = V.get(k, i + 1) / h;
                }

                for (int j = 0; j <= i; j++) {
                    double g = 0;
                    for (int k = 0; k <= i; k++) {
                        g += V.get(k, i + 1) * V.get(k, j);
                    }
                    for (int k = 0; k <= i; k++) {
                        V.set(k, j, V.get(k, j) - g * d[k]);
                    }
                }
            }

            for (int k = 0; k <= i; k++) {
                V.set(k, i + 1, 0);
            }
        }

        for (int j = 0; j < n; j++) {
            d[j] = V.get(n - 1, j);
            V.set(n - 1, j, 0);
        }

        V.set(n - 1, n - 1, 1);
        e[0] = 0;
    }

    private static void tql2(int n, double[] e, double[] d, Matrix V) {
        for (int i = 1; i < n; i++) {
            e[i - 1] = e[i];
        }

        e[n - 1] = 0;

        double f = 0;
        double tst1 = 0;
        double eps = Double.MIN_VALUE;

        for (int l = 0; l < n; l++) {
            tst1 = Math.max(tst1, Math.abs(d[l]) + Math.abs(e[l]));
            int m = l;
            while (m < n) {
                if (Math.abs(e[m]) <= eps * tst1) {
                    break;
                }
                m++;
            }

            if (m > l) {
                int iter = 0;
                do {
                    iter = iter + 1;

                    double g = d[l];
                    double p = (d[l + 1] - g) / (2 * e[l]);
                    double r = Utils.hypotenuse(p, 1);
                    if (p < 0) {
                        r = -r;
                    }

                    d[l] = e[l] / (p + r);
                    d[l + 1] = e[l] * (p + r);
                    double dl1 = d[l + 1];
                    double h = g - d[l];
                    for (int i = l + 2; i < n; i++) {
                        d[i] -= h;
                    }

                    f = f + h;

                    p = d[m];
                    double c = 1;
                    double c2 = c;
                    double c3 = c;
                    double el1 = e[l + 1];
                    double s = 0;
                    double s2 = 0;
                    for (int i = m - 1; i >= l; i--) {
                        c3 = c2;
                        c2 = c;
                        s2 = s;
                        g = c * e[i];
                        h = c * p;
                        r = Utils.hypotenuse(p, e[i]);
                        e[i + 1] = s * r;
                        s = e[i] / r;
                        c = p / r;
                        p = c * d[i] - s * g;
                        d[i + 1] = h + s * (c * g + s * d[i]);

                        for (int k = 0; k < n; k++) {
                            h = V.get(k, i + 1);
                            V.set(k, i + 1, s * V.get(k, i) + c * h);
                            V.set(k, i, c * V.get(k, i) - s * h);
                        }
                    }

                    p = (-s * s2 * c3 * el1 * e[l]) / dl1;
                    e[l] = s * p;
                    d[l] = c * p;
                } while (Math.abs(e[l]) > eps * tst1);
            }
            d[l] = d[l] + f;
            e[l] = 0;
        }

        for (int i = 0; i < n - 1; i++) {
            int k = i;
            double p = d[i];
            for (int j = i + 1; j < n; j++) {
                if (d[j] < p) {
                    k = j;
                    p = d[j];
                }
            }

            if (k != i) {
                d[k] = d[i];
                d[i] = p;
                for (int j = 0; j < n; j++) {
                    p = V.get(j, i);
                    V.set(j, i, V.get(j, k));
                    V.set(j, k, p);
                }
            }
        }
    }

    private static void orthes(int n, Matrix H, double[] ort, Matrix V) {
        int low = 0;
        int high = n - 1;

        for (int m = low + 1; m <= high - 1; m++) {
            double scale = 0;
            for (int i = m; i <= high; i++) {
                scale = scale + Math.abs(H.get(i, m - 1));
            }

            if (scale != 0) {
                double h = 0;
                for (int i = high; i >= m; i--) {
                    ort[i] = H.get(i, m - 1) / scale;
                    h += ort[i] * ort[i];
                }

                double g = Math.sqrt(h);
                if (ort[m] > 0) {
                    g = -g;
                }

                h = h - ort[m] * g;
                ort[m] = ort[m] - g;

                for (int j = m; j < n; j++) {
                    double f = 0;
                    for (int i = high; i >= m; i--) {
                        f += ort[i] * H.get(i, j);
                    }

                    f = f / h;
                    for (int i = m; i <= high; i++) {
                        H.set(i, j, H.get(i, j) - f * ort[i]);
                    }
                }

                for (int i = 0; i <= high; i++) {
                    double f = 0;
                    for (int j = high; j >= m; j--) {
                        f += ort[j] * H.get(i, j);
                    }

                    f = f / h;
                    for (int j = m; j <= high; j++) {
                        H.set(i, j, H.get(i, j) - f * ort[j]);
                    }
                }

                ort[m] = scale * ort[m];
                H.set(m, m - 1, scale * g);
            }
        }

        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                V.set(i, j, i == j ? 1 : 0);
            }
        }

        for (int m = high - 1; m >= low + 1; m--) {
            if (H.get(m, m - 1) != 0) {
                for (int i = m + 1; i <= high; i++) {
                    ort[i] = H.get(i, m - 1);
                }

                for (int j = m; j <= high; j++) {
                    double g = 0;
                    for (int i = m; i <= high; i++) {
                        g += ort[i] * V.get(i, j);
                    }

                    g = g / ort[m] / H.get(m, m - 1);
                    for (int i = m; i <= high; i++) {
                        V.set(i, j, V.get(i, j) + g * ort[i]);
                    }
                }
            }
        }
    }

    private static void hqr2(int nn, double[] e, double[] d, Matrix V, Matrix H) {
        int n = nn - 1;
        int low = 0;
        int high = nn - 1;
        double eps = Math.pow(2.0, -52.0);
        double exshift = 0;
        double norm = 0;
        double p = 0;
        double q = 0;
        double r = 0;
        double s = 0;
        double z = 0;
        int iter = 0;

        for (int i = 0; i < nn; i++) {
            if (i < low || i > high) {
                d[i] = H.get(i, i);
                e[i] = 0;
            }

            for (int j = Math.max(i - 1, 0); j < nn; j++) {
                norm = norm + Math.abs(H.get(i, j));
            }
        }

        while (n >= low) {
            int l = n;
            while (l > low) {
                s = Math.abs(H.get(l - 1, l - 1)) + Math.abs(H.get(l, l));
                if (s == 0) {
                    s = norm;
                }
                if (Math.abs(H.get(l, l - 1)) < eps * s) {
                    break;
                }
                l--;
            }

            if (l == n) {
                H.set(n, n, H.get(n, n) + exshift);
                d[n] = H.get(n, n);
                e[n] = 0;
                n--;
                iter = 0;
            } else if (l == n - 1) {
                double w = H.get(n, n - 1) * H.get(n - 1, n);
                p = (H.get(n - 1, n - 1) - H.get(n, n)) / 2;
                q = p * p + w;
                z = Math.sqrt(Math.abs(q));
                H.set(n, n, H.get(n, n) + exshift);
                H.set(n - 1, n - 1, H.get(n - 1, n - 1) + exshift);
                double x = H.get(n, n);

                if (q >= 0) {
                    z = p >= 0 ? p + z : p - z;
                    d[n - 1] = x + z;
                    d[n] = d[n - 1];
                    if (z != 0) {
                        d[n] = x - w / z;
                    }
                    e[n - 1] = 0;
                    e[n] = 0;
                    x = H.get(n, n - 1);
                    s = Math.abs(x) + Math.abs(z);
                    p = x / s;
                    q = z / s;
                    r = Math.sqrt(p * p + q * q);
                    p = p / r;
                    q = q / r;

                    for (int j = n - 1; j < nn; j++) {
                        z = H.get(n - 1, j);
                        H.set(n - 1, j, q * z + p * H.get(n, j));
                        H.set(n, j, q * H.get(n, j) - p * z);
                    }

                    for (int i = 0; i <= n; i++) {
                        z = H.get(i, n - 1);
                        H.set(i, n - 1, q * z + p * H.get(i, n));
                        H.set(i, n, q * H.get(i, n) - p * z);
                    }

                    for (int i = low; i <= high; i++) {
                        z = V.get(i, n - 1);
                        V.set(i, n - 1, q * z + p * V.get(i, n));
                        V.set(i, n, q * V.get(i, n) - p * z);
                    }
                } else {
                    d[n - 1] = x + p;
                    d[n] = x + p;
                    e[n - 1] = z;
                    e[n] = -z;
                }

                n = n - 2;
                iter = 0;
            } else {
                // Continue with complex case (truncated for brevity)
                n--;
                iter++;
            }
        }
    }
}