/**************************************************************************
 *
 * Copyright 2008 VMware, Inc.
 * All Rights Reserved.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a
 * copy of this software and associated documentation files (the
 * "Software"), to deal in the Software without restriction, including
 * without limitation the rights to use, copy, modify, merge, publish,
 * distribute, sub license, and/or sell copies of the Software, and to
 * permit persons to whom the Software is furnished to do so, subject to
 * the following conditions:
 *
 * The above copyright notice and this permission notice (including the
 * next paragraph) shall be included in all copies or substantial portions
 * of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
 * OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NON-INFRINGEMENT.
 * IN NO EVENT SHALL VMWARE AND/OR ITS SUPPLIERS BE LIABLE FOR
 * ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
 * TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
 * SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 *
 **************************************************************************/

/**
 * @file
 *
 * Fake WGL gallium frontend.
 *
 * These functions implement the WGL API, on top of the ICD DDI, so that the
 * resulting DLL can be used as a drop-in replacement for the system's
 * opengl32.dll.
 *
 * These functions never get called for ICD drivers, which use exclusively the
 * ICD DDI, i.e., the Drv* entrypoints.
 */

#include <windows.h>
#include <GL/gl.h>

#include "util/u_debug.h"
#include "stw_gdishim.h"
#include "stw_wgl.h"
#include "gldrv.h"
#include "stw_context.h"
#include "stw_pixelformat.h"
#include "stw_ext_context.h"

WINGDIAPI BOOL APIENTRY
wglCopyContext(
   HGLRC hglrcSrc,
   HGLRC hglrcDst,
   UINT mask )
{
   return DrvCopyContext( (DHGLRC)(UINT_PTR)hglrcSrc,
                          (DHGLRC)(UINT_PTR)hglrcDst,
                          mask );
}

WINGDIAPI HGLRC APIENTRY
wglCreateContext(
   HDC hdc )
{
   stw_override_opengl32_entry_points(&wglCreateContext, &wglDeleteContext);
   return (HGLRC)(UINT_PTR)DrvCreateContext(hdc);
}

WINGDIAPI HGLRC APIENTRY
wglCreateLayerContext(
   HDC hdc,
   int iLayerPlane )
{
   stw_override_opengl32_entry_points(&wglCreateContext, &wglDeleteContext);
   return (HGLRC)(UINT_PTR)DrvCreateLayerContext( hdc, iLayerPlane );
}

WINGDIAPI BOOL APIENTRY
wglDeleteContext(
   HGLRC hglrc )
{
   DrvReleaseContext((DHGLRC)(UINT_PTR)hglrc);
   return DrvDeleteContext((DHGLRC)(UINT_PTR)hglrc );
}


WINGDIAPI HGLRC APIENTRY
wglGetCurrentContext( VOID )
{
   return (HGLRC)(UINT_PTR)stw_get_current_context();
}

WINGDIAPI HDC APIENTRY
wglGetCurrentDC( VOID )
{
   return stw_get_current_dc();
}


WINGDIAPI BOOL APIENTRY
wglMakeCurrent(
   HDC hdc,
   HGLRC hglrc )
{
   return DrvSetContext( hdc, (DHGLRC)(UINT_PTR)hglrc, NULL ) ? true : false;
}


WINGDIAPI BOOL APIENTRY
wglSwapBuffers(
   HDC hdc )
{
   return DrvSwapBuffers( hdc );
}


WINGDIAPI DWORD WINAPI
wglSwapMultipleBuffers(UINT n,
                       CONST WGLSWAP *ps)
{
   UINT i;

   for (i =0; i < n; ++i)
      wglSwapBuffers(ps->hdc);

   return 0;
}


WINGDIAPI BOOL APIENTRY
wglSwapLayerBuffers(
   HDC hdc,
   UINT fuPlanes )
{
   return DrvSwapLayerBuffers( hdc, fuPlanes );
}

WINGDIAPI PROC APIENTRY
wglGetProcAddress(
    LPCSTR lpszProc )
{
   return DrvGetProcAddress( lpszProc );
}


WINGDIAPI int APIENTRY
wglChoosePixelFormat(
   HDC hdc,
   CONST PIXELFORMATDESCRIPTOR *ppfd )
{
   if (ppfd->nSize != sizeof( PIXELFORMATDESCRIPTOR ) || ppfd->nVersion != 1)
      return 0;
   if (ppfd->iPixelType != PFD_TYPE_RGBA)
      return 0;
   if (!(ppfd->dwFlags & PFD_DRAW_TO_WINDOW))
      return 0;
   if (!(ppfd->dwFlags & PFD_SUPPORT_OPENGL))
      return 0;
   if (ppfd->dwFlags & PFD_DRAW_TO_BITMAP)
      return 0;
   if (!(ppfd->dwFlags & PFD_STEREO_DONTCARE) && (ppfd->dwFlags & PFD_STEREO))
      return 0;

   return stw_pixelformat_choose( hdc, ppfd );
}

WINGDIAPI int APIENTRY
wglDescribePixelFormat(
   HDC hdc,
   int iPixelFormat,
   UINT nBytes,
   LPPIXELFORMATDESCRIPTOR ppfd )
{
   return DrvDescribePixelFormat( hdc, iPixelFormat, nBytes, ppfd );
}

WINGDIAPI int APIENTRY
wglGetPixelFormat(
   HDC hdc )
{
   return stw_pixelformat_get( hdc );
}

WINGDIAPI BOOL APIENTRY
wglSetPixelFormat(
   HDC hdc,
   int iPixelFormat,
   const PIXELFORMATDESCRIPTOR *ppfd )
{
    /* SetPixelFormat (hence wglSetPixelFormat) must not touch ppfd, per
     * http://msdn.microsoft.com/en-us/library/dd369049(v=vs.85).aspx
     */
   (void) ppfd;

   return DrvSetPixelFormat( hdc, iPixelFormat );
}


WINGDIAPI BOOL APIENTRY
wglUseFontBitmapsA(
   HDC hdc,
   DWORD first,
   DWORD count,
   DWORD listBase )
{
   return wglUseFontBitmapsW(hdc, first, count, listBase);
}

WINGDIAPI BOOL APIENTRY
wglShareLists(
   HGLRC hglrc1,
   HGLRC hglrc2 )
{
   return DrvShareLists((DHGLRC)(UINT_PTR)hglrc1,
                        (DHGLRC)(UINT_PTR)hglrc2);
}

WINGDIAPI BOOL APIENTRY
wglUseFontBitmapsW(
   HDC hdc,
   DWORD first,
   DWORD count,
   DWORD listBase )
{
#if !defined _GAMING_XBOX && !defined _XBOX_UWP
   GLYPHMETRICS gm;
   MAT2 tra;
   FIXED one, minus_one, zero;
   void *buffer = NULL;
   BOOL result = true;

   one.value = 1;
   one.fract = 0;
   minus_one.value = -1;
   minus_one.fract = 0;
   zero.value = 0;
   zero.fract = 0;

   tra.eM11 = one;
   tra.eM22 = minus_one;
   tra.eM12 = tra.eM21 = zero;

   for (int i = 0; i < count; i++) {
      DWORD size = GetGlyphOutline(hdc, first + i, GGO_BITMAP, &gm, 0,
                                   NULL, &tra);

      glNewList(listBase + i, GL_COMPILE);

      if (size != GDI_ERROR) {
         if (size == 0) {
            glBitmap(0, 0, (GLfloat)-gm.gmptGlyphOrigin.x,
                     (GLfloat)gm.gmptGlyphOrigin.y,
                     (GLfloat)gm.gmCellIncX,
                     (GLfloat)gm.gmCellIncY, NULL);
         }
         else {
            buffer = realloc(buffer, size);
            size = GetGlyphOutline(hdc, first + i, GGO_BITMAP, &gm,
                                   size, buffer, &tra);

            glBitmap(gm.gmBlackBoxX, gm.gmBlackBoxY,
                     -gm.gmptGlyphOrigin.x, gm.gmptGlyphOrigin.y,
                     gm.gmCellIncX, gm.gmCellIncY, buffer);
         }
      }
      else {
         result = false;
      }

      glEndList();
   }

   free(buffer);

   return result;
#else
   return false;
#endif /* _GAMING_XBOX && !defined _XBOX_UWP */
}

WINGDIAPI BOOL APIENTRY
wglUseFontOutlinesA(
   HDC hdc,
   DWORD first,
   DWORD count,
   DWORD listBase,
   FLOAT deviation,
   FLOAT extrusion,
   int format,
   LPGLYPHMETRICSFLOAT lpgmf )
{
   (void) hdc;
   (void) first;
   (void) count;
   (void) listBase;
   (void) deviation;
   (void) extrusion;
   (void) format;
   (void) lpgmf;

   assert( 0 );

   return false;
}

WINGDIAPI BOOL APIENTRY
wglUseFontOutlinesW(
   HDC hdc,
   DWORD first,
   DWORD count,
   DWORD listBase,
   FLOAT deviation,
   FLOAT extrusion,
   int format,
   LPGLYPHMETRICSFLOAT lpgmf )
{
   (void) hdc;
   (void) first;
   (void) count;
   (void) listBase;
   (void) deviation;
   (void) extrusion;
   (void) format;
   (void) lpgmf;

   assert( 0 );

   return false;
}

WINGDIAPI BOOL APIENTRY
wglDescribeLayerPlane(
   HDC hdc,
   int iPixelFormat,
   int iLayerPlane,
   UINT nBytes,
   LPLAYERPLANEDESCRIPTOR plpd )
{
   return DrvDescribeLayerPlane(hdc, iPixelFormat, iLayerPlane, nBytes, plpd);
}

WINGDIAPI int APIENTRY
wglSetLayerPaletteEntries(
   HDC hdc,
   int iLayerPlane,
   int iStart,
   int cEntries,
   CONST COLORREF *pcr )
{
   return DrvSetLayerPaletteEntries(hdc, iLayerPlane, iStart, cEntries, pcr);
}

WINGDIAPI int APIENTRY
wglGetLayerPaletteEntries(
   HDC hdc,
   int iLayerPlane,
   int iStart,
   int cEntries,
   COLORREF *pcr )
{
   return DrvGetLayerPaletteEntries(hdc, iLayerPlane, iStart, cEntries, pcr);
}

WINGDIAPI BOOL APIENTRY
wglRealizeLayerPalette(
   HDC hdc,
   int iLayerPlane,
   BOOL bRealize )
{
   (void) hdc;
   (void) iLayerPlane;
   (void) bRealize;

   assert( 0 );

   return false;
}


