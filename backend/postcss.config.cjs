// FE-003: Docker image-time CSS flattening — flattens CSS Nesting Level 1
// for browsers without native support (Firefox <120, Safari <16.5).
// Source files in backend/static/*.css remain nested (author input);
// flattened output is written to backend/static/dist/*.css during `docker build`.
module.exports = {
  plugins: [
    require('postcss-preset-env')({
      stage: 1,
      features: {
        'nesting-rules': true,
      },
      // Preserve other preset-env defaults; only nesting is critical for FE-003.
      autoprefixer: false,
    }),
  ],
};
