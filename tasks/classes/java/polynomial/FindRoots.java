package polynomial;

import utils.Utils;

public class FindRoots {
    
    private static boolean near(double a, double b, double c, double d, double tol) {
        double qa = a - c;
        double qb = b - d;
        double r = qa * qa + qb * qb;
        return r * r < tol;
    }

    private static double[] solve(int n, int nIters, double tolerance, double[] zr, double[] zi, double[] pr, double[] pi) {
        final double EPSILON = 1e-8;

        int m = zr.length;
        
        for (int i = 0; i < nIters; i++) {
            double d = 0.0;
            for (int j = 0; j < m; j++) {
                // Read in zj
                double pa = zr[j];
                double pb = zi[j];

                // Compute denominator
                // (zj - z0) * (zj - z1) * ... * (zj - z_{n-1})
                double a = 1.0;
                double b = 0.0;
                for (int k = 0; k < m; k++) {
                    if (k == j) {
                        continue;
                    }
                    double qa = pa - zr[k];
                    double qb = pb - zi[k];
                    if (qa * qa + qb * qb < tolerance) {
                        continue;
                    }
                    double k1 = qa * (a + b);
                    double k2 = a * (qb - qa);
                    double k3 = b * (qa + qb);
                    a = k1 - k3;
                    b = k1 + k2;
                }

                // Compute numerator
                double na = pr[n - 1];
                double nb = pi[n - 1];
                double s1 = pb - pa;
                double s2 = pa + pb;
                for (int k = n - 2; k >= 0; k--) {
                    double k1 = pa * (na + nb);
                    double k2 = na * s1;
                    double k3 = nb * s2;
                    na = k1 - k3 + pr[k];
                    nb = k1 + k2 + pi[k];
                }

                // Compute reciprocal
                double k1 = a * a + b * b;
                if (Math.abs(k1) > EPSILON) {
                    a /= k1;
                    b /= -k1;
                } else {
                    a = 1.0;
                    b = 0.0;
                }

                // Multiply and accumulate
                k1 = na * (a + b);
                double k2 = a * (nb - na);
                double k3 = b * (na + nb);

                double qa = k1 - k3;
                double qb = k1 + k2;

                zr[j] = pa - qa;
                zi[j] = pb - qb;

                d = Math.max(d, Math.max(Math.abs(qa), Math.abs(qb)));
            }

            // If converged, exit early
            if (d < tolerance) {
                break;
            }
        }

        // Post process: Combine any repeated roots
        for (int i = 0; i < m; i++) {
            int count = 1;
            double a = zr[i];
            double b = zi[i];
            for (int j = 0; j < m; j++) {
                if (i == j) {
                    continue;
                }
                if (near(zr[i], zi[i], zr[j], zi[j], tolerance)) {
                    count++;
                    a += zr[j];
                    b += zi[j];
                }
            }
            if (count > 1) {
                a /= count;
                b /= count;
                for (int j = 0; j < m; j++) {
                    if (i == j) {
                        continue;
                    }
                    if (near(zr[i], zi[i], zr[j], zi[j], tolerance)) {
                        zr[j] = a;
                        zi[j] = b;
                    }
                }
                zr[i] = a;
                zi[i] = b;
            }
        }
        
        // Return both arrays concatenated
        double[] result = new double[zr.length + zi.length];
        System.arraycopy(zr, 0, result, 0, zr.length);
        System.arraycopy(zi, 0, result, zr.length, zi.length);
        return result;
    }

    private static double bound(int n, double[] pr, double[] pi) {
        double b = 0.0;
        for (int i = 0; i < n; i++) {
            b = Math.max(b, pr[i] * pr[i] + pi[i] * pi[i]);
        }
        return 1.0 + Math.sqrt(b);
    }

    public static double[][] findRoots(double[] rCoeff) {
        int n = rCoeff.length;
        if (n <= 1) {
            return new double[][]{new double[0], new double[0]};
        }
        
        int nl = Utils.nextPow2(n);
        double[] pr = new double[nl];
        double[] pi = new double[nl];

        System.arraycopy(rCoeff, 0, pr, 0, n);
        for (int i = 0; i < n; i++) {
            pi[i] = 0.0;
        }

        // Rescale coefficients
        double a = pr[n - 1];
        double b = pi[n - 1];
        double d = a * a + b * b;
        a /= d;
        b /= -d;
        double s = b - a;
        double t = a + b;
        
        for (int i = 0; i < n - 1; i++) {
            double k1 = a * (pr[i] + pi[i]);
            double k2 = pr[i] * s;
            double k3 = pi[i] * t;
            pr[i] = k1 - k3;
            pi[i] = k1 + k2;
        }
        pr[n - 1] = 1.0;
        pi[n - 1] = 0.0;
        
        int nIters = 100 * n;
        double tolerance = 1e-6;
        
        // Pick default initial guess
        double[] zr = new double[n - 1];
        double[] zi = new double[n - 1];
        double r = bound(n, pr, pi);
        
        for (int i = 0; i < n - 1; i++) {
            double radius = Math.random() * r;
            double c = Math.cos(Math.random() * 2 * Math.PI);
            zr[i] = radius * c;
            zi[i] = radius * Math.sqrt(1.0 - c * c);
        }
        
        double[] result = solve(n, nIters, tolerance, zr, zi, pr, pi);
        
        // Split result back into real and imaginary parts
        double[] realParts = new double[n - 1];
        double[] imagParts = new double[n - 1];
        System.arraycopy(result, 0, realParts, 0, n - 1);
        System.arraycopy(result, n - 1, imagParts, 0, n - 1);
        
        return new double[][]{realParts, imagParts};
    }
}