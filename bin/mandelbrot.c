#include "syscall.h"

double fabs(double x) {
  if (x < 0.0) {
    return -x;
  } else {
    return x;
  }
}

int main() {
  /* screen ( integer) coordinate */
  int iX, iY;
  const int iXmax = 500;
  const int iYmax = 500;
  /* world ( double) coordinate = parameter plane*/
  double Cx, Cy;
  const double CxMin = -2.5;
  const double CxMax = 1.5;
  const double CyMin = -2.0;
  const double CyMax = 2.0;
  /* */
  double PixelWidth = (CxMax - CxMin) / iXmax;
  double PixelHeight = (CyMax - CyMin) / iYmax;
  /* color component ( R or G or B) is coded from 0 to 255 */
  /* it is 24 bit color RGB file */
  const int MaxColorComponentValue = 255;
  static unsigned char color[3];
  /* Z=Zx+Zy*i  ;   Z0 = 0 */
  double Zx, Zy;
  double Zx2, Zy2; /* Zx2=Zx*Zx;  Zy2=Zy*Zy  */
  /*  */
  int Iteration;
  const int IterationMax = 200;
  /* bail-out value , radius of circle ;  */
  const double EscapeRadius = 2;
  double ER2 = EscapeRadius * EscapeRadius;

  int width = iXmax;
  int height = iYmax;
  int window_id = create_window("mandelbrot", 10, 10, 10, width, height);
  unsigned long buf_addr = 0x10000000;
  int err = map_window(window_id, buf_addr);

  /*create new file,give it a name and open it in binary mode  */
  for (iY = 0; iY < iYmax; iY++) {
    Cy = CyMin + iY * PixelHeight;
    if (fabs(Cy) < PixelHeight / 2)
      Cy = 0.0; /* Main antenna */
    for (iX = 0; iX < iXmax; iX++) {
      Cx = CxMin + iX * PixelWidth;
      /* initial value of orbit = critical point Z= 0 */
      Zx = 0.0;
      Zy = 0.0;
      Zx2 = Zx * Zx;
      Zy2 = Zy * Zy;
      /* */
      for (Iteration = 0; Iteration < IterationMax && ((Zx2 + Zy2) < ER2);
           Iteration++) {
        Zy = 2 * Zx * Zy + Cy;
        Zx = Zx2 - Zy2 + Cx;
        Zx2 = Zx * Zx;
        Zy2 = Zy * Zy;
      };
      /* compute  pixel color (24 bit = 3 bytes) */
      if (Iteration == IterationMax) { /*  interior of Mandelbrot set = black */
        color[0] = 0;
        color[1] = 0;
        color[2] = 0;
      } else {          /* exterior of Mandelbrot set = white */
        color[0] = 255; /* Red*/
        color[1] = 255; /* Green */
        color[2] = 255; /* Blue */
      };
      /*write color to the file*/
      // fwrite(color,1,3,fp);
      *((unsigned char *)(buf_addr + (iY * width + iX) * 4)) = color[0];
      *((unsigned char *)(buf_addr + (iY * width + iX) * 4) + 1) = color[1];
      *((unsigned char *)(buf_addr + (iY * width + iX) * 4) + 2) = color[2];
      *((unsigned char *)(buf_addr + (iY * width + iX) * 4) + 3) = 0x00;
    }
  }
  sync_window(window_id);
  return 0;
}