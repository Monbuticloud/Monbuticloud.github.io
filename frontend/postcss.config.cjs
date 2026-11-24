// FE-003: Docker image-time CSS flattening — flattens CSS Nesting Level 1
// for browsers without native support (Firefox <120, Safari <16.5).
// Source files in frontend/static/*.css remain nested (author input);
// flattened output is written to frontend/static/dist/*.css during `docker build`.
//
// `stage: false` disables every other preset-env polyfill so the build only
// flattens nesting; `nesting-rules` is enabled explicitly below.
module.exports = {
  plugins: [
    require('postcss-preset-env')({
      stage: false,
      features: {
        'nesting-rules': true,
      },
      autoprefixer: false,
    }),
  ],
};
