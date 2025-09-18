package polynomial;

import java.util.HashMap;
import java.util.Map;

public class Polynomial {
    private final Map<Integer, Double> coeffs;

    public Polynomial(Map<Integer, Double> coeffs) {
        this.coeffs = new HashMap<>(coeffs);
    }

    public Map<Integer, Double> getCoeffs() {
        return new HashMap<>(coeffs);
    }

    private Map<Integer, Double> cloneCoeffs() {
        Map<Integer, Double> result = new HashMap<>();
        for (Map.Entry<Integer, Double> entry : coeffs.entrySet()) {
            result.put(entry.getKey(), entry.getValue());
        }
        return result;
    }

    public int degree() {
        int max = -1;
        for (Map.Entry<Integer, Double> entry : coeffs.entrySet()) {
            if (entry.getValue() != 0) {
                max = Math.max(max, entry.getKey());
            }
        }
        return max;
    }

    public double[] realCoeffs() {
        int deg = degree();
        double[] realCoeffs = new double[deg + 1];
        for (int i = 0; i < deg + 1; i++) {
            realCoeffs[i] = coeffs.getOrDefault(i, 0.0);
        }
        return realCoeffs;
    }
}